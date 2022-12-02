use crate::board::{Sensor, SensorServo};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue, ExtEvent};
use crate::system_time::{Duration, Ticker};

use calibration::Calibration;
use core::cell::{RefCell, RefMut};
use fugit::ExtU32;
use rtt_target::rprintln;
use vl53l1x::{DistanceMode, TimingBudget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveResult {
    SameDirection,
    ChangeDirection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScanMode {
    Baseline(Calibration),
    ScanDown,
    ScanUp,
}

struct State {
    ticker: Ticker,
    sensor: Sensor,
    servo: SensorServo,
    mode: ScanMode,
    current_step: i32,
    total_steps: i32,
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

const SENSOR_TIMING_BUDGET: Duration = Duration::millis(100);
const SENSOR_RETRY_TIME: Duration = Duration::millis(10);
const SERVO_RESET_TIME: Duration = Duration::millis(500);
const SERVO_STEP_TIME: Duration = Duration::millis(100);

static STATE: StaticState = StaticState::new();

static START_RANGING: Event = Event::new(&|| start_ranging().unwrap());
static READ_SENSOR: Event = Event::new(&|| read_sensor().unwrap());

// Preconditions: servo positioned, sensor off.
// Postcondition: sensor started.
fn start_ranging() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    state.sensor.start_ranging()?;
    READ_SENSOR.call_at(state.ticker.now() + SENSOR_TIMING_BUDGET);

    Ok(())
}

fn read_sensor() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    if state.sensor.check_for_data_ready()? {
        let distance = state.sensor.get_distance()?;
        state.sensor.clear_interrupt()?;

        match state.mode {
            ScanMode::Baseline(_) => {
                state.sensor.stop_ranging()?;
                process_calibration(distance);
                move_servo(state);
            }
            _ => {
                state.sensor.stop_ranging()?;
                process_scan(distance);
                move_servo(state);
            }
        }
    } else {
        rprintln!("sensor not ready");
        // Try again shortly
        READ_SENSOR.call_at(state.ticker.now() + SENSOR_RETRY_TIME);
    }

    Ok(())
}

fn process_calibration(distance: u16) {
    rprintln!("cal {}", distance);
}

fn process_scan(distance: u16) {
    rprintln!("run {}", distance);
}

fn move_servo(state: &mut State) -> MoveResult {
    let mut step = 0;

    #[allow(clippy::collapsible_else_if)]
    if state.mode == ScanMode::ScanDown {
        if state.current_step == 0 {
            state.mode = ScanMode::ScanUp;
        } else {
            step = -1;
        }
    } else {
        if state.current_step == state.total_steps - 1 {
            state.mode = ScanMode::ScanDown;
        } else {
            step = 1;
        }
    }

    if step != 0 {
        state.current_step += step;

        let fraction = state.current_step as f32 / state.total_steps as f32;
        state.servo.fraction(fraction);

        START_RANGING.call_at(state.ticker.now() + SERVO_STEP_TIME);
        MoveResult::SameDirection
    } else {
        START_RANGING.call();
        MoveResult::ChangeDirection
    }
}

pub fn start(
    ticker: Ticker,
    event_queue: &mut EventQueue<'_, '_, 'static>,
    mut sensor: Sensor,
    mut servo: SensorServo,
) -> Result<(), Error> {
    servo.fraction(0.0);

    sensor.set_timing_budget(TimingBudget::Ms100)?;
    sensor.set_distance_mode(DistanceMode::Long)?;
    sensor.set_inter_measurement(200.millis())?;

    *STATE.get() = Some(State {
        ticker,
        sensor,
        servo,
        mode: ScanMode::Baseline(Calibration::new()),
        current_step: 0,
        total_steps: 50, // TODO set from the pot angle
    });

    event_queue.bind(&START_RANGING);
    event_queue.bind(&READ_SENSOR);
    START_RANGING.call_at(ticker.now() + SERVO_RESET_TIME);

    Ok(())
}
