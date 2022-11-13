#![deny(unsafe_code)]
#![no_std]
#![no_main]

use crate::event_queue::{Event, ExtEvent};
use cortex_m_rt::entry;
use fugit::ExtU32;
use rtt_target::rprintln;
use rtt_target::rtt_init_print;
use servo::{Bounds, Servo};
use stm32f1xx_hal::adc;
use stm32f1xx_hal::i2c::{I2c, Mode};
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::time::{Hertz, MilliSeconds};
use stm32f1xx_hal::timer::Timer;
use vl53l1x::{BootState, DistanceMode, TimingBudget, VL53L1X};

use panic_probe as _;
// use panic_halt as _;

mod event_queue;
mod system_time;

const SERVO_FREQ: Hertz = Hertz::Hz(50);

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

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
    let servo_range = adc.read(&mut servo_range_ch).unwrap();
    let max_range = adc.max_sample();
    // adc.release(); // No longer needed

    rprintln!("range {} of {}", servo_range, max_range);

    // Disable JTAG to get PB3 (mistake in board design)
    let (_, pb3, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);

    let mut led = pb3.into_push_pull_output(&mut gpiob.crl);
    let button = gpiob.pb5.into_pull_down_input(&mut gpiob.crl);

    let sensor_servo_pin = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let laser_servo_pin = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
    let mut laser_pin = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);

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

    let period: MilliSeconds = SERVO_FREQ.try_into_duration().unwrap();
    let period_ms = period.to_millis().try_into().unwrap();

    let bounds = Bounds::scale_from_period_ms(&sensor_servo_pwm, period_ms, servo_range, max_range);
    let mut sensor_servo = Servo::new(sensor_servo_pwm, bounds);
    sensor_servo.percent(50);
    sensor_servo.enable();

    let bounds = Bounds::scale_from_period_ms(&laser_servo_pwm, period_ms, servo_range, max_range);
    let mut laser_servo = Servo::new(laser_servo_pwm, bounds);
    laser_servo.percent(50);
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

    let ticker = system_time::Ticker::new(Timer::syst(cp.SYST, &clocks));
    let mut sensor = VL53L1X::new(i2c, vl53l1x::ADDR);

    while sensor.boot_state().unwrap() != BootState::Booted {
        // Wait 10 ms until next timer tick
        cortex_m::asm::wfi();
    }

    sensor.sensor_init().unwrap();
    sensor.set_timing_budget(TimingBudget::Ms100).unwrap();
    sensor.set_distance_mode(DistanceMode::Long).unwrap();
    sensor.set_inter_measurement(200.millis()).unwrap();
    sensor.start_ranging().unwrap();

    let mut handler = || {
        let range: u32 = adc.read(&mut servo_range_ch).unwrap();
        let max_range = adc.max_sample() as u32;
        let pct = 100 * range / max_range;
        let b = button.is_high();

        rprintln!("range {} pct {} button {}", range, pct, b);
        if sensor.check_for_data_ready().unwrap() {
            let distance = sensor.get_distance().unwrap();
            let status = sensor.get_range_status().unwrap();

            rprintln!("distance {} status {}", distance, status);
        } else {
            rprintln!("sensor data not ready");
        }

        sensor_servo.percent(pct as u8);
        laser_servo.percent(pct as u8);
        if b {
            laser_pin.set_high();
        } else {
            laser_pin.set_low();
        }
        led.toggle();
    };

    let mut event = Event::new_mut(&mut handler);
    event.set_period(500.millis());
    event.call();

    let mut queue = event_queue::EventQueue::new(&ticker);

    queue.bind(&event);
    queue.run_forever();
}
