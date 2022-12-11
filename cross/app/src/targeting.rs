use crate::board::{Laser, LaserServo, Led};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue, ExtEvent};
use crate::system_time::{Duration, Instant, Ticker};

use core::cell::{RefCell, RefMut};
use core::cmp::{max, min};
use num::rational::Ratio;
use num::Zero;
use rtt_target::rprintln;

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

    fn get(&self) -> RefMut<Option<State>> {
        self.state.borrow_mut()
    }
}

// STATE is only accessed from the main thread via EventQueue.
// Therefore, no locking is necessary.
unsafe impl Sync for StaticState {}

static STATE: StaticState = StaticState::new();

static LASER_OFF: Event = Event::new(&|| laser_off().unwrap());
static TARGET_LOST: Event = Event::new(&target_lost);

fn laser_off() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    state.laser.set_low();
    state.last_lock = state.ticker.now();

    rprintln!("AUDIO: contact lost");
    TARGET_LOST.call_at(state.ticker.now() + TARGET_LOST_DELAY);

    Ok(())
}

fn target_lost() {
    rprintln!("AUDIO: target lost");
}

pub fn start(
    ticker: Ticker,
    event_queue: &mut EventQueue<'_, 'static>,
    led: Led,
    laser: Laser,
    mut servo: LaserServo,
    total_steps: u16,
) -> Result<(), Error> {
    servo.set(Ratio::zero())?;

    *STATE.get() = Some(State {
        target_state: TargetState::NoContact,
        last_lock: Instant::from_ticks(0),
        ticker,
        led,
        laser,
        servo,
        total_steps,
    });

    event_queue.bind(&LASER_OFF);
    event_queue.bind(&TARGET_LOST);

    Ok(())
}

pub fn reset() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    state.target_state = TargetState::NoContact;

    Ok(())
}

fn set_lock(state: &mut State, start_position: u16, end_position: u16) -> Result<(), Error> {
    state.target_state = TargetState::Lock {
        start_position,
        end_position,
    };

    let low_side = min(start_position, end_position);
    let high_side = max(start_position, end_position);

    let servo_position = Ratio::new(low_side + (high_side - low_side) / 2, state.total_steps);

    state.servo.set(servo_position)?;
    state.laser.set_high();

    LASER_OFF.call_at(state.ticker.now() + LASER_OFF_DELAY);
    TARGET_LOST.cancel();

    Ok(())
}

pub fn report(position: u16, contact: bool) -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    if contact {
        state.led.set_high();

        match state.target_state {
            TargetState::NoContact => {
                state.target_state = TargetState::EarlyContact {
                    start_position: position,
                };
            }
            TargetState::EarlyContact { start_position } => {
                let low_side = min(start_position, position);
                let high_side = max(start_position, position);

                if high_side - low_side == MIN_TARGET_LOCK_RANGE {
                    if state.ticker.now() - state.last_lock >= TARGET_ACQUIRED_INTERVAL {
                        rprintln!("AUDIO: target acquired");
                    } else {
                        rprintln!("AUDIO: contact restored");
                    }
                    set_lock(state, start_position, position)?;
                }
            }
            TargetState::Lock {
                start_position,
                end_position: _,
            } => {
                set_lock(state, start_position, position)?;
            }
        }
    } else {
        state.led.set_low();

        match state.target_state {
            TargetState::NoContact => {}
            TargetState::EarlyContact { start_position: _ } => {
                state.target_state = TargetState::NoContact;
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
                    state.target_state = TargetState::NoContact;
                }
            }
        }
    }

    Ok(())
}
