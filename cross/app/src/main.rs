#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_probe as _;
// use panic_halt as _;

use crate::event_queue::{Event, ExtEvent};
use cortex_m_rt::entry;
use rtt_target::rprintln;
use rtt_target::rtt_init_print;
use servo::{Bounds, Servo};
use stm32f1xx_hal::adc;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::time::{Hertz, MilliSeconds};

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
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    let mut afio = dp.AFIO.constrain();

    // Acquire the GPIO peripherals.
    let mut gpioa = dp.GPIOA.split();
    let mut gpioc = dp.GPIOC.split();

    // Read servo range calibration value
    let mut adc = adc::Adc::adc1(dp.ADC1, clocks);
    let mut servo_range_ch = gpioa.pa0.into_analog(&mut gpioa.crl);
    let servo_range = adc.read(&mut servo_range_ch).unwrap();
    let max_range = adc.max_sample();
    adc.release(); // No longer needed

    rprintln!("range {} of {}", servo_range, max_range);

    let mut led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);
    let button = gpioc.pc13.into_floating_input(&mut gpioc.crh);

    let sensor_servo_pin = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let laser_servo_pin = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);

    let (sensor_servo_pwm, _laser_servo_pwm) = dp
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

    let mut handler = || {
        let pct = if button.is_low() { 0 } else { 100 };

        sensor_servo.percent(pct);
        led.toggle();
    };

    let mut event = Event::new_mut(&mut handler);
    event.set_period(500.millis());
    event.call();

    let ticker = system_time::Ticker::new(cp.SYST);
    let mut queue = event_queue::EventQueue::new(&ticker);

    queue.bind(&event);
    queue.run_forever();
}
