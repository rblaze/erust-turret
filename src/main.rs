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
use stm32f1xx_hal::time::{Hertz, MicroSeconds};

pub mod servo;

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
            Hertz::Hz(50),
            &clocks,
        )
        .split();

    // Calculate 1ms and 2ms bounds for servo
    let lower_bound = sensor_servo_pwm.get_max_duty() / 20; // 20ms (50Hz) / 20 = 1ms
    let upper_bound = sensor_servo_pwm.get_max_duty() / 10; // 20ms (50Hz) / 10 = 2ms

    let mut sensor_servo = servo::Servo::new(sensor_servo_pwm, lower_bound, upper_bound);
    sensor_servo.percent(50);
    sensor_servo.enable();

    loop {
        rprintln!("loop");
        sleep_timer.delay(MicroSeconds::millis(1000));
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
