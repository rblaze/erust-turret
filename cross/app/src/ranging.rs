use crate::board::{Sensor, SensorServo};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue, ExtEvent};
use crate::system_time::{Duration, Ticker};
use crate::targeting::Targeting;

use calibration::Calibration;
use core::cell::{RefCell, RefMut};
use num::rational::Ratio;
use num::{One, Zero};
use rtt_target::rprintln;
use vl53l1x::{DistanceMode, TimingBudget};

const MAX_STEPS: usize = 100;
const NUM_CALIBRATION_SAMPLES: u16 = 5;

const SENSOR_TIMING_BUDGET: Duration = Duration::millis(100);
const SENSOR_INTERMEASURMENT_TIME: Duration = Duration::millis(120);
const SENSOR_RETRY_TIME: Duration = Duration::millis(10);
const SERVO_RESET_TIME: Duration = Duration::millis(500);
const SERVO_STEP_TIME: Duration = Duration::millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveResult {
    SameDirection,
    ChangeDirection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CalibrationResult {
    NeedMoreData,
    Done(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScanMode {
    Baseline(Calibration),
    ScanDown,
    ScanUp,
}

struct Ranging {
    targeting: Targeting,
    ticker: Ticker,
    sensor: Sensor,
    servo: SensorServo,
    mode: ScanMode,
    current_step: usize,
    total_steps: usize,
    baseline: [u16; MAX_STEPS],
}

impl Ranging {
    fn init(
        ticker: Ticker,
        mut sensor: Sensor,
        mut servo: SensorServo,
        total_steps: usize,
        targeting: Targeting,
    ) -> Result<Self, Error> {
        sensor.set_timing_budget(TimingBudget::Ms100)?;
        sensor.set_distance_mode(DistanceMode::Long)?;
        sensor.set_inter_measurement(SENSOR_INTERMEASURMENT_TIME.convert())?;

        servo.set(Ratio::zero())?;
        START_RANGING.call_at(ticker.now() + SERVO_RESET_TIME);

        Ok(Ranging {
            ticker,
            sensor,
            servo,
            mode: ScanMode::Baseline(Calibration::new()),
            current_step: 0,
            total_steps,
            baseline: [0; MAX_STEPS],
            targeting,
        })
    }

    fn start_measurement(&mut self) -> Result<(), Error> {
        self.sensor.start_ranging()?;
        READ_SENSOR.call_at(self.ticker.now() + SENSOR_TIMING_BUDGET);

        Ok(())
    }

    fn read_sensor(&mut self) -> Result<(), Error> {
        if !(self.sensor.check_for_data_ready()?) {
            rprintln!("sensor not ready");
            // Try again shortly
            READ_SENSOR.call_at(self.ticker.now() + SENSOR_RETRY_TIME);
            return Ok(());
        }

        let distance = self.sensor.get_distance()?;
        self.sensor.clear_interrupt()?;

        if let ScanMode::Baseline(ref mut calibration) = self.mode {
            if let CalibrationResult::Done(threshold) =
                Self::process_calibration(calibration, distance)
            {
                self.baseline[self.current_step] = threshold;
                self.mode = ScanMode::Baseline(Calibration::new());
                self.sensor.stop_ranging()?;
                self.move_servo()?;
            } else {
                // Get next scan in 200 ms
                READ_SENSOR.call_at(self.ticker.now() + SENSOR_INTERMEASURMENT_TIME);
            }
        } else {
            self.process_scan(distance)?;
            self.sensor.stop_ranging()?;

            if self.move_servo()? == MoveResult::ChangeDirection {
                self.targeting.reset()?;
            }
        }

        Ok(())
    }

    fn process_calibration(calibration: &mut Calibration, distance: u16) -> CalibrationResult {
        rprintln!("cal {}", distance);
        calibration.add_sample(distance);

        if calibration.num_samples() == NUM_CALIBRATION_SAMPLES {
            let point = calibration.get_point();

            let buffer = 3 * point.stddev;
            let threshold = if point.mean > buffer {
                point.mean - buffer
            } else {
                0
            };

            rprintln!("point {:?} threshold {}", point, threshold);

            CalibrationResult::Done(threshold)
        } else {
            CalibrationResult::NeedMoreData
        }
    }

    fn process_scan(&self, distance: u16) -> Result<(), Error> {
        rprintln!("run {}", distance);

        self.targeting.report(
            self.current_step as u16,
            distance < self.baseline[self.current_step],
        )
    }

    fn move_servo(&mut self) -> Result<MoveResult, Error> {
        let mut result = MoveResult::SameDirection;

        #[allow(clippy::collapsible_else_if)]
        if self.mode == ScanMode::ScanDown {
            if self.current_step == 0 {
                self.mode = ScanMode::ScanUp;
                result = MoveResult::ChangeDirection;
            } else {
                self.current_step -= 1;
            }
        } else {
            if self.current_step == self.total_steps - 1 {
                self.mode = ScanMode::ScanDown;
                result = MoveResult::ChangeDirection;
            } else {
                self.current_step += 1;
            }
        }

        if result == MoveResult::SameDirection {
            self.servo.set(Ratio::new(
                self.current_step as u16,
                self.total_steps as u16,
            ))?;

            START_RANGING.call_at(self.ticker.now() + SERVO_STEP_TIME);
        } else {
            START_RANGING.call();
        }

        Ok(result)
    }
}

struct StaticState {
    state: RefCell<Option<Ranging>>,
}

impl StaticState {
    const fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }

    fn get(&self) -> RefMut<Option<Ranging>> {
        self.state.borrow_mut()
    }
}

// STATE is only accessed from the main thread via EventQueue.
// Therefore, no locking is necessary.
unsafe impl Sync for StaticState {}

static STATE: StaticState = StaticState::new();

static START_RANGING: Event = Event::new(&|| start_ranging().unwrap());
static READ_SENSOR: Event = Event::new(&|| read_sensor().unwrap());

// Preconditions: servo positioned, sensor off.
// Postcondition: sensor started.
fn start_ranging() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    state.start_measurement()
}

fn read_sensor() -> Result<(), Error> {
    let mut stref = STATE.get();
    let state = stref.as_mut().ok_or(Error::Uninitialized)?;

    state.read_sensor()
}

pub fn get_num_steps_from_angle_scale(scale: Ratio<u16>) -> Result<usize, Error> {
    if scale > Ratio::one() {
        return Err(Error::InvalidScale);
    }

    let long_scale = Ratio::new((*scale.numer()).into(), (*scale.denom()).into());
    let total_steps = (Ratio::from_integer(MAX_STEPS) * long_scale).to_integer();
    rprintln!("using {} steps", total_steps);

    Ok(total_steps)
}

pub fn start(
    ticker: Ticker,
    event_queue: &mut EventQueue<'_, 'static>,
    sensor: Sensor,
    servo: SensorServo,
    num_steps: usize,
    targeting: Targeting,
) -> Result<(), Error> {
    event_queue.bind(&START_RANGING);
    event_queue.bind(&READ_SENSOR);

    *STATE.get() = Some(Ranging::init(ticker, sensor, servo, num_steps, targeting)?);

    Ok(())
}
