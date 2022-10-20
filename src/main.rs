#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_probe as _;
// use panic_halt as _;

use cortex_m_rt::entry;
use rtt_target::rprintln;
use rtt_target::rtt_init_print;
use stm32f1xx_hal::gpio::PinState;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::time::{Hertz, MilliSeconds};

pub mod servo;

const SERVO_FREQ: Hertz = Hertz::Hz(50);

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Configure the clock.
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    let mut sleep_timer = cp.SYST.delay(&clocks);

    let mut afio = dp.AFIO.constrain();

    // Acquire the GPIO peripherals.
    let mut gpioa = dp.GPIOA.split();
    let mut gpioc = dp.GPIOC.split();

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

    let period: MilliSeconds = SERVO_FREQ.try_into_duration().unwrap();
    let bounds =
        servo::Bounds::from_period_ms(&sensor_servo_pwm, period.to_millis().try_into().unwrap());

    let mut sensor_servo = servo::Servo::new(sensor_servo_pwm, bounds);
    sensor_servo.percent(50);
    sensor_servo.enable();

    loop {
        rprintln!("loop");
        sleep_timer.delay(1.secs());
        let state;
        let pct;

        if button.is_low() {
            state = PinState::High;
            pct = 0;
        } else {
            state = PinState::Low;
            pct = 100;
        };

        led.set_state(state);
        sensor_servo.percent(pct);
    }
}
