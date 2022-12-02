use crate::board::{Sensor, SensorServo};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue, ExtEvent};
use crate::system_time::{Duration, Ticker};

use calibration::Calibration;
use core::cell::{RefCell, RefMut};
use rtt_target::rprintln;
use vl53l1x::{DistanceMode, TimingBudget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveResult {
    SameDirection,
    ChangeDirection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CalibrationResult {
    NeedMorePoints,
    Done,
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
const SENSOR_INTERMEASURMENT_TIME: Duration = Duration::millis(200);
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

    if !(state.sensor.check_for_data_ready()?) {
        rprintln!("sensor not ready");
        // Try again shortly
        READ_SENSOR.call_at(state.ticker.now() + SENSOR_RETRY_TIME);
        return Ok(());
    }

    let distance = state.sensor.get_distance()?;
    state.sensor.clear_interrupt()?;

    if let ScanMode::Baseline(ref mut calibration) = state.mode {
        if process_calibration(calibration, distance) == CalibrationResult::Done {
            state.mode = ScanMode::Baseline(Calibration::new());
            state.sensor.stop_ranging()?;
            move_servo(state);
        } else {
            // Get next scan in 200 ms
            READ_SENSOR.call_at(state.ticker.now() + SENSOR_INTERMEASURMENT_TIME);
        }
    } else {
        process_scan(distance);
        state.sensor.stop_ranging()?;
        move_servo(state);
    }

    Ok(())
}

fn process_calibration(calibration: &mut Calibration, distance: u16) -> CalibrationResult {
    rprintln!("cal {}", distance);
    calibration.add_sample(distance);

    if calibration.num_samples() == 3 {
        let point = calibration.get_point();
        rprintln!("point {:?}", point);

        CalibrationResult::Done
    } else {
        CalibrationResult::NeedMorePoints
    }
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
    sensor.set_inter_measurement(SENSOR_INTERMEASURMENT_TIME.convert())?;

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
