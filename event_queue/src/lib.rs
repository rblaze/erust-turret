#![cfg_attr(not(test), no_std)]

use core::cell::Cell;
use core::cell::RefCell;
use core::fmt::{Debug, Formatter, Result};
use core::ops::DerefMut;
use critical_section::Mutex;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

pub type TICKS = u32;

#[derive(Debug)]
pub struct EventQueue<'e, 'h> {
    events: LinkedList<EventAdapter<'e, 'h>>,
}

intrusive_adapter!(EventAdapter<'e, 'h> = &'e Event<'h>: Event<'h> { link: LinkedListLink });

impl<'e, 'h> EventQueue<'e, 'h> {
    pub fn new() -> Self {
        EventQueue {
            events: LinkedList::new(EventAdapter::new()),
        }
    }

    pub fn bind(&mut self, event: &'e Event<'h>) {
        self.events.push_back(event);
    }

    // Check all registered events once and execute all pending handlers.
    pub fn run_once(&self, ticks: TICKS) {
        let mut cursor = self.events.front();

        loop {
            match cursor.get() {
                None => break,
                Some(event) => {
                    let dispatch = critical_section::with(|cs| {
                        let state = *event.state.borrow_ref(cs);
                        let period = event.period.borrow(cs).get();

                        let (dispatch, event_time) = match state {
                            EventState::Done => (false, ticks),
                            EventState::DispatchNow => (true, ticks),
                            EventState::DispatchAt(dispatch_time) => {
                                (dispatch_time <= ticks, dispatch_time)
                            }
                        };

                        if dispatch {
                            match period {
                                None => event.state.replace(cs, EventState::Done),
                                Some(duration) => event
                                    .state
                                    .replace(cs, EventState::DispatchAt(event_time + duration)),
                            };
                        }

                        dispatch
                    });

                    if dispatch {
                        match event.handler.borrow_mut().deref_mut() {
                            Handler::Fn(h) => h(),
                            Handler::FnMut(h) => h(),
                        }
                    }

                    cursor.move_next();
                }
            }
        }
    }
}

impl<'e, 'h> Default for EventQueue<'e, 'h> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventState {
    Done,
    DispatchNow,
    DispatchAt(TICKS),
}

enum Handler<'h> {
    Fn(&'h dyn Fn()),
    FnMut(&'h mut dyn FnMut()),
}

impl<'h> Debug for Handler<'h> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Handler::Fn(_) => f.write_str("Handler::Fn(_)"),
            Handler::FnMut(_) => f.write_str("Handler::FnMut(_)"),
        }
    }
}

pub struct Event<'h> {
    // Only changes in EventQueue::bind(), no locking necessary.
    link: LinkedListLink,
    // Protected.
    state: Mutex<RefCell<EventState>>,
    // Protected.
    period: Mutex<Cell<Option<TICKS>>>,
    // Never changes, no locking necessary.
    handler: RefCell<Handler<'h>>,
}

impl Debug for Event<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Event")
            .field(
                "state",
                &critical_section::with(|cs| *self.state.borrow_ref(cs)),
            )
            .field(
                "period",
                &critical_section::with(|cs| self.period.borrow(cs).get()),
            )
            .finish()
    }
}

unsafe impl<'h> Sync for Event<'h> {}

impl<'h> Event<'h> {
    pub const fn new(handler: &'h dyn Fn()) -> Self {
        Self {
            link: LinkedListLink::new(),
            state: Mutex::new(RefCell::new(EventState::Done)),
            period: Mutex::new(Cell::new(None)),
            handler: RefCell::new(Handler::Fn(handler)),
        }
    }

    pub fn new_mut(handler: &'h mut dyn FnMut()) -> Self {
        Self {
            link: LinkedListLink::new(),
            state: Mutex::new(RefCell::new(EventState::Done)),
            period: Mutex::new(Cell::new(None)),
            handler: RefCell::new(Handler::FnMut(handler)),
        }
    }

    /// Cancel dispatch of the event.
    /// This function is interrupt-safe.
    pub fn cancel(&self) {
        critical_section::with(|cs| {
            self.state.replace(cs, EventState::Done);
        });
    }

    /// Post event into message queue for immediate dispatch.
    /// This function is interrupt-safe.
    pub fn call(&self) {
        critical_section::with(|cs| {
            self.state.replace(cs, EventState::DispatchNow);
        });
    }

    /// Post an event into message queue with a delay before dispatching the event.
    /// This function is interrupt-safe.
    pub fn call_on(&self, time: TICKS) {
        critical_section::with(|cs| {
            self.state.replace(cs, EventState::DispatchAt(time));
        });
    }

    /// Set period for repeatedly dispatching an event.
    /// This function is interrupt-safe.
    pub fn period(&self, period: TICKS) {
        critical_section::with(|cs| {
            self.period.borrow(cs).set(Some(period));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn test_fn_handler() {
        let done = Cell::new(false);

        let handler = || {
            done.set(true);
        };

        let event = Event::new(&handler);
        let mut queue = EventQueue::new();

        queue.bind(&event);
        event.call();
        queue.run_once(0);

        assert!(done.get());
    }

    #[test]
    fn test_fnmut_handler() {
        let mut done = false;
        {
            let mut handler = || {
                done = true;
            };

            let event = Event::new_mut(&mut handler);
            let mut queue = EventQueue::new();

            queue.bind(&event);
            event.call();
            queue.run_once(0);
        }
        assert!(done);
    }

    #[test]
    fn test_post_multiple_times() {
        let done = RefCell::new(0);

        let handler = || {
            done.replace_with(|n| *n + 1);
        };

        let event = Event::new(&handler);
        let mut queue = EventQueue::new();
        queue.bind(&event);

        event.call();
        assert_eq!(*done.borrow(), 0);

        queue.run_once(0);
        assert_eq!(*done.borrow(), 1);

        queue.run_once(100);
        assert_eq!(*done.borrow(), 1);

        event.call();
        queue.run_once(200);
        assert_eq!(*done.borrow(), 2);
    }

    #[test]
    fn test_delayed_post() {
        let done = Cell::new(false);

        let handler = || {
            done.set(true);
        };

        let event = Event::new(&handler);
        let mut queue = EventQueue::new();

        queue.bind(&event);
        event.call_on(100);

        queue.run_once(0);
        assert!(!done.get());

        queue.run_once(50);
        assert!(!done.get());

        queue.run_once(100);
        assert!(done.get());

        done.set(false);

        // Check that handler doesn't run again.
        queue.run_once(110);
        assert!(!done.get());
    }

    #[test]
    fn test_periodic_event() {
        let done = RefCell::new(0);

        let handler = || {
            done.replace_with(|n| *n + 1);
        };

        let event = Event::new(&handler);
        event.period(100);

        let mut queue = EventQueue::new();
        queue.bind(&event);

        event.call();
        assert_eq!(*done.borrow(), 0);

        queue.run_once(7);
        assert_eq!(*done.borrow(), 1);

        queue.run_once(106);
        assert_eq!(*done.borrow(), 1);

        queue.run_once(107);
        assert_eq!(*done.borrow(), 2);

        queue.run_once(210);
        assert_eq!(*done.borrow(), 3);
    }
}

#[cfg(test)]
mod static_tests {
    use super::*;

    static DONE: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

    fn handler() {
        critical_section::with(|cs| {
            DONE.borrow(cs).set(true);
        });
    }

    static EVENT: Event = Event::new(&handler);

    #[test]
    fn test_post_static_event() {
        let mut queue = EventQueue::new();

        queue.bind(&EVENT);
        EVENT.call();
        queue.run_once(0);

        let done = critical_section::with(|cs| DONE.borrow(cs).get());

        assert!(done);
    }
}
