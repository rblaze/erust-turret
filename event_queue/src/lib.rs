#![cfg_attr(not(test), no_std)]

use core::cell::Cell;
use core::cell::RefCell;
use critical_section::Mutex;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

// Millisecond-precision time. Good for 49 days before rollover.
type Instant = fugit::Instant<u32, 1, 1000>;
type Duration = fugit::MillisDurationU32;

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
    pub fn run_once(&mut self, time: Instant) {
        let mut cursor = self.events.front();

        loop {
            match cursor.get() {
                None => break,
                Some(event) => {
                    let state = critical_section::with(|cs| *event.state.borrow_ref(cs));

                    let dispatch = match state {
                        EventState::Done => false,
                        EventState::DispatchNow => true,
                        EventState::DispatchAt(dispatch_time) => dispatch_time <= time,
                    };

                    if dispatch {
                        critical_section::with(|cs| {
                            event.state.replace(cs, EventState::Done);
                        });

                        event.handler.borrow()();
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
    DispatchAt(Instant),
}

pub struct Event<'h> {
    // Only changes in EventQueue::bind()
    link: LinkedListLink,
    state: Mutex<RefCell<EventState>>,
    period: Mutex<Cell<Option<Duration>>>,
    handler: RefCell<&'h dyn Fn()>, // Never changes
}

unsafe impl<'h> Sync for Event<'h> {}

impl<'h> Event<'h> {
    pub const fn new(handler: &'h dyn Fn()) -> Self {
        Self {
            link: LinkedListLink::new(),
            state: Mutex::new(RefCell::new(EventState::Done)),
            period: Mutex::new(Cell::new(None)),
            handler: RefCell::new(handler),
        }
    }

    // Post event into message queue for immediate dispatch.
    // This function is interrupt-safe.
    pub fn call(&self) {
        critical_section::with(|cs| {
            self.state.replace(cs, EventState::DispatchNow);
        });
    }

    // Post an event into message queue with a delay before dispatching the event.
    // This function is interrupt-safe.
    pub fn call_at(&self, time: Instant) {
        critical_section::with(|cs| {
            self.state.replace(cs, EventState::DispatchAt(time));
        });
    }

    // Set period for repeatedly dispatching an event.
    // This function is interrupt-safe.
    pub fn period(&mut self, period: Duration) {
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
    fn test_post_event() {
        let done = Cell::new(false);

        let handler = || {
            done.set(true);
        };

        let event = Event::new(&handler);
        let mut queue = EventQueue::new();

        queue.bind(&event);
        event.call();
        queue.run_once(Instant::from_ticks(0));

        assert!(done.get());
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

        queue.run_once(Instant::from_ticks(0));
        assert_eq!(*done.borrow(), 1);

        queue.run_once(Instant::from_ticks(100));
        assert_eq!(*done.borrow(), 1);

        event.call();
        queue.run_once(Instant::from_ticks(200));
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
        event.call_at(Instant::from_ticks(100));

        queue.run_once(Instant::from_ticks(0));
        assert!(!done.get());

        queue.run_once(Instant::from_ticks(50));
        assert!(!done.get());

        queue.run_once(Instant::from_ticks(100));
        assert!(done.get());

        done.set(false);

        // Check that handler doesn't run again.
        queue.run_once(Instant::from_ticks(110));
        assert!(!done.get());
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
        queue.run_once(Instant::from_ticks(0));

        let done = critical_section::with(|cs| DONE.borrow(cs).get());

        assert!(done);
    }
}
