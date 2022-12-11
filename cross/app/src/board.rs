#![deny(unsafe_code)]

use crate::error::Error;
use crate::system_time::Ticker;

use num::rational::Ratio;
use rtt_target::rprintln;
use servo::{Bounds, Servo};
use stm32f1xx_hal::device::{I2C1, TIM1};
use stm32f1xx_hal::gpio::{Alternate, Input, Output};
use stm32f1xx_hal::gpio::{OpenDrain, PullDown, PushPull};
use stm32f1xx_hal::gpio::{PA5, PA8, PA9, PB3, PB5, PB6, PB7};
use stm32f1xx_hal::i2c::{BlockingI2c, I2c, Mode};
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::time::{Hertz, MilliSeconds};
use stm32f1xx_hal::timer::{PwmChannel, Timer};
use stm32f1xx_hal::{adc, pac};
use vl53l1x::{BootState, VL53L1X};

const SERVO_FREQ: Hertz = Hertz::Hz(50);

type I2cPins = (PB6<Alternate<OpenDrain>>, PB7<Alternate<OpenDrain>>);
type I2cBus = BlockingI2c<I2C1, I2cPins>;
pub type Sensor = VL53L1X<I2cBus>;
type SensorServoPin = PA8<Alternate<PushPull>>;
pub type SensorServo = Servo<PwmChannel<TIM1, 0>>;

type Laser = PA5<Output<PushPull>>;
type LaserServoPin = PA9<Alternate<PushPull>>;
type LaserServo = Servo<PwmChannel<TIM1, 1>>;

type Led = PB3<Output<PushPull>>;
type Button = PB5<Input<PullDown>>;

pub struct Board {
    pub ticker: Ticker,
    pub laser_led: Laser,
    pub laser_servo: LaserServo,
    pub sensor: Sensor,
    pub sensor_servo: SensorServo,
    pub target_lock_led: Led,
    pub button: Button,
    pub adc_ratio: Ratio<u16>,
}

impl Board {
    pub fn new(cp: pac::CorePeripherals, dp: pac::Peripherals) -> Result<Self, Error> {
        // Enable debug while sleeping to keep probe-rs happy while WFI
        dp.DBGMCU.cr.modify(|_, w| {
            w.dbg_sleep().set_bit();
            w.dbg_standby().set_bit();
            w.dbg_stop().set_bit()
        });
        dp.RCC.ahbenr.modify(|_, w| w.dma1en().enabled());

        // Configure the clock.
        let mut flash = dp.FLASH.constrain();
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(64.MHz()).freeze(&mut flash.acr);

        let mut afio = dp.AFIO.constrain();

        // Acquire the GPIO peripherals.
        let mut gpioa = dp.GPIOA.split();
        let mut gpiob = dp.GPIOB.split();

        // Read servo range calibration value
        let mut adc = adc::Adc::adc1(dp.ADC1, clocks);
        let mut servo_range_ch = gpioa.pa1.into_analog(&mut gpioa.crl);
        let mut adc_value = adc.read(&mut servo_range_ch)?;
        let adc_max = adc.max_sample();
        adc.release(); // No longer needed

        rprintln!("range {} of {}", adc_value, adc_max);
        // Avoid too small range
        if adc_value < adc_max / 10 {
            adc_value = adc_max / 10;
        }

        let adc_ratio = Ratio::new(adc_value, adc_max);

        // Disable JTAG to get PB3 (mistake in board design)
        let (_, pb3, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);

        let target_lock_led = pb3.into_push_pull_output(&mut gpiob.crl);
        let button = gpiob.pb5.into_pull_down_input(&mut gpiob.crl);
        let laser_led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);

        let sensor_servo_pin: SensorServoPin = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
        let laser_servo_pin: LaserServoPin = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);

        let (sensor_servo_pwm, laser_servo_pwm) = dp
            .TIM1
            .pwm_hz(
                (sensor_servo_pin, laser_servo_pin),
                &mut afio.mapr,
                SERVO_FREQ,
                &clocks,
            )
            .split();

        rprintln!("pwm max duty {}", sensor_servo_pwm.get_max_duty());

        let period: MilliSeconds = SERVO_FREQ
            .try_into_duration()
            .ok_or(Error::InvalidDuration)?;
        let period_ms = period.to_millis().try_into()?;

        let bounds = Bounds::scale_from_period_ms(&sensor_servo_pwm, period_ms, adc_ratio)?;
        rprintln!("sensor {}", bounds);
        let mut sensor_servo = Servo::new(sensor_servo_pwm, bounds);
        sensor_servo.enable();

        let bounds = Bounds::scale_from_period_ms(&laser_servo_pwm, period_ms, adc_ratio)?;
        let mut laser_servo = Servo::new(laser_servo_pwm, bounds);
        laser_servo.enable();

        let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
        let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);
        let i2c = I2c::i2c1(
            dp.I2C1,
            (scl, sda),
            &mut afio.mapr,
            Mode::standard(100.kHz()),
            clocks,
        )
        .blocking_default(clocks);

        let ticker = Ticker::new(Timer::syst(cp.SYST, &clocks));
        let mut sensor = VL53L1X::new(i2c, vl53l1x::ADDR);

        while sensor.boot_state()? != BootState::Booted {
            // Wait 10 ms until next timer tick. Ticker must be enabled.
            cortex_m::asm::wfi();
        }
        sensor.sensor_init()?;

        Ok(Board {
            ticker,
            laser_led,
            laser_servo,
            sensor,
            sensor_servo,
            target_lock_led,
            button,
            adc_ratio,
        })
    }
}
