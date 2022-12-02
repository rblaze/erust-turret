#![no_std]
#![no_main]

mod board;
mod error;
mod event_queue;
mod ranging;
mod system_time;

use crate::board::Board;
use cortex_m_rt::entry;
use rtt_target::rtt_init_print;
use stm32f1xx_hal::pac;

use panic_probe as _;
// use panic_halt as _;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let board = Board::new(cp, dp).unwrap();
    let mut queue = event_queue::EventQueue::new(&board.ticker);

    ranging::start(board.ticker, &mut queue, board.sensor, board.sensor_servo).unwrap();

    queue.run_forever();
}
