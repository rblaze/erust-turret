#![deny(unsafe_code)]
#![no_std]
#![no_main]

// use panic_probe as _;
use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::gpio::PinState;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    // Configure the clock.
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let _ = rcc.cfgr.freeze(&mut flash.acr);

    // Acquire the GPIO peripherals.
    let mut gpioa = dp.GPIOA.split();
    let mut gpioc = dp.GPIOC.split();

    // Configure PA5 as output.
    let mut led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);

    // Configure PC13 as input.
    let button = gpioc.pc13.into_floating_input(&mut gpioc.crh);

    loop {
        let state = if button.is_low() { PinState::High } else { PinState::Low };

        led.set_state(state);
    }
}
