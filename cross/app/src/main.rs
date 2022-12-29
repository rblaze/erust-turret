#![no_std]
#![no_main]

mod audio;
mod board;
mod error;
mod event_queue;
mod ranging;
mod system_time;
mod targeting;

use crate::audio::Audio;
use crate::board::Board;
use crate::targeting::Targeting;
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
    let mut queue = event_queue::EventQueue::new(board.ticker);

    let audio = Audio::new();

    let num_steps = ranging::get_num_steps_from_angle_scale(board.adc_ratio).unwrap();

    let targeting = Targeting::new(
        board.ticker,
        &mut queue,
        board.target_lock_led,
        board.laser_led,
        board.laser_servo,
        num_steps as u16,
        audio,
    )
    .unwrap();

    ranging::start(
        board.ticker,
        &mut queue,
        board.sensor,
        board.sensor_servo,
        num_steps,
        targeting,
        audio,
    )
    .unwrap();

    queue.run_forever();
}
