#![no_std]
#![no_main]

mod board;
mod error;
mod event_queue;
mod ranging;
mod system_time;
mod targeting;

use crate::board::Board;
use cortex_m_rt::entry;
use num::rational::Ratio;
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
    let mut queue = event_queue::EventQueue::new(board.ticker);

    let num_steps = ranging::start(
        board.ticker,
        &mut queue,
        board.sensor,
        board.sensor_servo,
        Ratio::new(
            *board.adc_ratio.numer() as usize,
            *board.adc_ratio.denom() as usize,
        ),
    )
    .unwrap();

    targeting::start(
        board.ticker,
        &mut queue,
        board.target_lock_led,
        board.laser_led,
        board.laser_servo,
        num_steps,
    )
    .unwrap();

    queue.run_forever();
}
