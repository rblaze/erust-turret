#![no_std]

use core::cell::RefCell;

use embedded_hal::blocking::spi::{Transfer, Write};
use embedded_hal::digital::v2::OutputPin;
use littlefs2::consts::{U4, U64};
use littlefs2::driver::Storage;
use littlefs2::io::Error;
use spi_memory::series25::Flash;
use spi_memory::{BlockDevice, Read};

pub struct SoundStorage<SPI, CS> {
    // Storage only provides &self for read() and spi_memory::Read requires &mut self.
    // Therefore, use runtime borrow here.
    flash: RefCell<Flash<SPI, CS>>,
}

impl<SPI, CS, E> Storage for SoundStorage<SPI, CS>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin,
{
    const READ_SIZE: usize = 16;
    const WRITE_SIZE: usize = 16;

    // Memory chip supports minimum 4KB sector erase
    const BLOCK_SIZE: usize = 4096;
    // W25Q16JL 16Mbit flash
    const BLOCK_COUNT: usize = 512;

    // Do some wear-leveling, though the application is read-only.
    const BLOCK_CYCLES: isize = 10;

    type CACHE_SIZE = U64;
    type LOOKAHEADWORDS_SIZE = U4;

    // Regardless of mutablity, none of these functions are interrupt-safe.
    fn read(&self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
        self.flash
            .borrow_mut()
            .read(off as u32, buf)
            .into_result(buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
        self.flash
            .get_mut()
            .write_bytes(off as u32, data)
            .into_result(data.len())
    }

    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        self.flash
            .get_mut()
            .erase_sectors(off as u32, len / Self::BLOCK_SIZE)
            .into_result(len)
    }
}

trait ToLfsResult {
    fn into_result(self, size: usize) -> Result<usize, Error>;
}

impl<SpiError, CsError> ToLfsResult for Result<(), spi_memory::Error<SpiError, CsError>> {
    fn into_result(self, size: usize) -> Result<usize, Error> {
        self.map(|_| size).map_err(|_| Error::Io)
    }
}
