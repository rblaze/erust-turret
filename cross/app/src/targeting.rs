use crate::audio::{Audio, Sound};
use crate::board::{Laser, LaserServo, Led};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue, ExtEvent};
use crate::system_time::{Duration, Instant, Ticker};

use core::cell::RefCell;
use core::cmp::{max, min};
use num::rational::Ratio;
use num::Zero;

const MIN_TARGET_LOCK_RANGE: u16 = 8;
const MAX_TARGET_BREAK_RANGE: u16 = 4;

const LASER_OFF_DELAY: Duration = Duration::secs(5);
const TARGET_LOST_DELAY: Duration = Duration::secs(60);
const TARGET_ACQUIRED_INTERVAL: Duration = Duration::secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TargetState {
    NoContact,
    EarlyContact {
        start_position: u16,
    },
    Lock {
        start_position: u16,
        end_position: u16,
    },
}

struct State {
    target_state: TargetState,
    last_lock: Instant,
    ticker: Ticker,
    led: Led,
    laser: Laser,
    servo: LaserServo,
    total_steps: u16,
    audio: Audio,
}

impl State {
    fn init(
        ticker: Ticker,
        led: Led,
        laser: Laser,
        mut servo: LaserServo,
        total_steps: u16,
        audio: Audio,
    ) -> Result<Self, Error> {
        servo.set(Ratio::zero())?;

        Ok(State {
            target_state: TargetState::NoContact,
            last_lock: Instant::from_ticks(0),
            ticker,
            led,
            laser,
            servo,
            total_steps,
            audio,
        })
    }

    fn reset(&mut self) {
        self.target_state = TargetState::NoContact;
    }

    fn laser_off(&mut self) {
        self.laser.set_low();
        self.last_lock = self.ticker.now();

        self.audio.play(Sound::ContactLost);
        TARGET_LOST.call_at(self.ticker.now() + TARGET_LOST_DELAY);
    }

    fn set_lock(&mut self, start_position: u16, end_position: u16) -> Result<(), Error> {
        self.target_state = TargetState::Lock {
            start_position,
            end_position,
        };

        let low_side = min(start_position, end_position);
        let high_side = max(start_position, end_position);

        let servo_position = Ratio::new(low_side + (high_side - low_side) / 2, self.total_steps);

        self.servo.set(servo_position)?;
        self.laser.set_high();

        LASER_OFF.call_at(self.ticker.now() + LASER_OFF_DELAY);
        TARGET_LOST.cancel();

        Ok(())
    }

    fn process_contact(&mut self, position: u16) -> Result<(), Error> {
        self.led.set_high();

        match self.target_state {
            TargetState::NoContact => {
                self.target_state = TargetState::EarlyContact {
                    start_position: position,
                };
            }
            TargetState::EarlyContact { start_position } => {
                let low_side = min(start_position, position);
                let high_side = max(start_position, position);

                if high_side - low_side == MIN_TARGET_LOCK_RANGE {
                    if self.ticker.now() - self.last_lock >= TARGET_ACQUIRED_INTERVAL {
                        self.audio.play(Sound::TargetAcquired);
                    } else {
                        self.audio.play(Sound::ContactRestored);
                    }
                    self.set_lock(start_position, position)?;
                }
            }
            TargetState::Lock {
                start_position,
                end_position: _,
            } => {
                self.set_lock(start_position, position)?;
            }
        }

        Ok(())
    }

    fn process_no_contact(&mut self, position: u16) -> Result<(), Error> {
        self.led.set_low();

        match self.target_state {
            TargetState::NoContact => {}
            TargetState::EarlyContact { start_position: _ } => {
                self.target_state = TargetState::NoContact;
            }
            TargetState::Lock {
                start_position,
                end_position,
            } => {
                let lock_break = if start_position < end_position {
                    position - end_position >= MAX_TARGET_BREAK_RANGE
                } else {
                    end_position - position >= MAX_TARGET_BREAK_RANGE
                };

                if lock_break {
                    self.target_state = TargetState::NoContact;
                }
            }
        }

        Ok(())
    }

    fn report(&mut self, position: u16, contact: bool) -> Result<(), Error> {
        if contact {
            self.process_contact(position)
        } else {
            self.process_no_contact(position)
        }
    }
}

struct StaticState {
    state: RefCell<Option<State>>,
}

impl StaticState {
    const fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }

    fn set(&self, state: State) {
        *self.state.borrow_mut() = Some(state);
    }

    fn with<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: Fn(&mut State) -> Result<R, Error>,
    {
        let mut stref = self.state.borrow_mut();
        let state = stref.as_mut().ok_or(Error::Uninitialized)?;

        f(state)
    }
}

// STATE is only accessed from the main thread via EventQueue.
// Therefore, no locking is necessary.
unsafe impl Sync for StaticState {}

pub struct Targeting;

impl Targeting {
    pub fn new(
        ticker: Ticker,
        event_queue: &mut EventQueue<'_, 'static>,
        led: Led,
        laser: Laser,
        servo: LaserServo,
        total_steps: u16,
        audio: Audio,
    ) -> Result<Self, Error> {
        event_queue.bind(&LASER_OFF);
        event_queue.bind(&TARGET_LOST);

        STATE.set(State::init(ticker, led, laser, servo, total_steps, audio)?);

        Ok(Targeting {})
    }

    // NOT interrupt-safe
    pub fn reset(&self) -> Result<(), Error> {
        STATE.with(|state| {
            state.reset();
            Ok(())
        })
    }

    // NOT interrupt-safe
    pub fn report(&self, position: u16, contact: bool) -> Result<(), Error> {
        STATE.with(|state| state.report(position, contact))
    }
}

static STATE: StaticState = StaticState::new();

static LASER_OFF: Event = Event::new(&|| {
    STATE
        .with(|state| {
            state.laser_off();
            Ok(())
        })
        .unwrap()
});
static TARGET_LOST: Event = Event::new(&|| {
    STATE
        .with(|state| {
            state.audio.play(Sound::TargetLost);
            Ok(())
        })
        .unwrap()
});
