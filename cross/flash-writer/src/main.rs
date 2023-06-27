#![no_std]
#![no_main]

mod board;
mod error;

use crate::board::Board;

use bytes::Buf;
use core::cmp::min;
use cortex_m::asm::wfi;
use cortex_m_rt::entry;
use nb::block;
use rtt_target::{rprintln, rtt_init_print};
use spi_memory::BlockDevice;
use stm32f1xx_hal::dma::ReadDma;
use stm32f1xx_hal::pac;

use panic_probe as _;

const BLOCK_LEN: usize = 4096;
static mut BLOCK: [u8; BLOCK_LEN + 4] = [0; BLOCK_LEN + 4];

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let mut board = Board::new(dp).unwrap();
    let mut rx = board.rx;
    let mut tx = board.tx;

    rprintln!("Press button to start");
    while board.button.is_low() {}

    rprintln!("Erasing flash...");
    board.memory.erase_all().unwrap();
    rprintln!("Flash erased");

    // Read total data length, u32be
    let mut total_len_buf = [0; 4];
    for byte in total_len_buf.iter_mut() {
        *byte = block!(rx.read()).unwrap();
    }
    let total_len = u32::from_be_bytes(total_len_buf) as usize;
    rprintln!("Expected image length {} bytes", total_len);

    // Send block length, u16be
    tx.bwrite_all((BLOCK_LEN as u16).to_be_bytes().as_ref())
        .unwrap();

    let mut rxdma = rx.with_dma(board.dma);
    let mut current_block = 0;
    while current_block * BLOCK_LEN < total_len {
        let bytes_left = total_len - current_block * BLOCK_LEN;
        let expected_bytes = min(BLOCK_LEN, bytes_left);
        rprintln!(
            "Reading block {} of {} bytes",
            current_block,
            expected_bytes
        );

        let buffer = unsafe { &mut BLOCK[..expected_bytes + 4] };
        // Read block from serial
        let (bytes, retrx) = rxdma.read(buffer).wait();
        rxdma = retrx;
        // Verify CRC
        let expected_crc = u32::from_be_bytes(bytes[expected_bytes..].try_into().unwrap());
        let mut data_bytes = &bytes[..expected_bytes];

        board.crc.reset();
        while data_bytes.remaining() > 4 {
            board.crc.write(data_bytes.get_u32());
        }
        if data_bytes.remaining() > 0 {
            let mut buf = [0; 4];
            let mut index = 0;
            while data_bytes.remaining() > 0 {
                buf[index] = data_bytes.get_u8();
                index += 1;
            }
            board.crc.write(u32::from_be_bytes(buf));
        }
        let actual_crc = board.crc.read();
        if actual_crc != expected_crc {
            panic!(
                "crc mismatch: received {}, calculated {}",
                expected_crc, actual_crc
            );
        }

        // Write to flash
        rprintln!("Writing block");

        // Send confirmation
        block!(tx.write(42)).unwrap();

        current_block += 1;
    }

    loop {
        wfi();
    }
}