#![deny(unsafe_code)]

use crate::system_time::{Duration, Instant, Ticker};
use cortex_m::asm::wfi;

pub use event_queue::Event;

pub trait ExtEvent {
    fn call_at(&self, instant: Instant);
    fn set_period(&mut self, period: Duration);
}

impl<'h> ExtEvent for Event<'h> {
    fn call_at(&self, instant: Instant) {
        self.call_on(instant.ticks());
    }

    fn set_period(&mut self, period: Duration) {
        self.period(period.ticks());
    }
}

pub struct EventQueue<'e, 'h> {
    queue: event_queue::EventQueue<'e, 'h>,
    ticker: Ticker,
}

impl<'e, 'h> EventQueue<'e, 'h> {
    pub fn new(ticker: Ticker) -> Self {
        EventQueue {
            queue: event_queue::EventQueue::new(),
            ticker,
        }
    }

    pub fn bind(&mut self, event: &'e Event<'h>) {
        self.queue.bind(event);
    }

    pub fn run_forever(self) -> ! {
        loop {
            self.queue.run_once(self.ticker.get_ticks());
            wfi();
        }
    }
}
