#![deny(unsafe_code)]

use crate::board::{SpiBus, SpiCs};

pub type SpiMemoryError = spi_memory::Error<SpiBus, SpiCs>;

#[derive(Debug)]
pub enum Error {
    SpiMemory(SpiMemoryError),
}

impl From<SpiMemoryError> for Error {
    fn from(error: SpiMemoryError) -> Self {
        Error::SpiMemory(error)
    }
}
