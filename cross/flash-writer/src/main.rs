#![no_std]
#![no_main]

mod board;
mod error;

use crate::board::Board;

use cortex_m::asm::wfi;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use spi_memory::BlockDevice;
use stm32f1xx_hal::pac;

use panic_probe as _;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let mut board = Board::new(dp).unwrap();

    rprintln!("Press button to start");
    while board.button.is_low() {}

    rprintln!("Erasing flash...");
    board.memory.erase_all().unwrap();
    rprintln!("Flash erased");

    // Read total data length
    // Read block length
    // Read block from serial
    // Verify CRC
    // Write to flash
    // Send confirmation

    loop {
        wfi();
    }
}
