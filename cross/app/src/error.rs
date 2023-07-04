#![deny(unsafe_code)]

use crate::storage::StorageError;
use core::num::TryFromIntError;

#[derive(Debug)]
pub enum Error {
    Servo(servo::Error),
    Sensor(vl53l1x::Error<stm32f1xx_hal::i2c::Error>),
    FileSystem(simplefs::Error<StorageError>),
    Timer(stm32f1xx_hal::timer::Error),
    InvalidDuration,
    InvalidScale,
    ConversionError(TryFromIntError),
    UnexpectedlyBlocks,
    Uninitialized,
}

impl From<servo::Error> for Error {
    fn from(servo_error: servo::Error) -> Self {
        Error::Servo(servo_error)
    }
}

impl From<vl53l1x::Error<stm32f1xx_hal::i2c::Error>> for Error {
    fn from(sensor_error: vl53l1x::Error<stm32f1xx_hal::i2c::Error>) -> Self {
        Error::Sensor(sensor_error)
    }
}

impl From<simplefs::Error<StorageError>> for Error {
    fn from(fs_error: simplefs::Error<StorageError>) -> Self {
        Error::FileSystem(fs_error)
    }
}

impl From<stm32f1xx_hal::timer::Error> for Error {
    fn from(timer_error: stm32f1xx_hal::timer::Error) -> Self {
        Error::Timer(timer_error)
    }
}

impl From<TryFromIntError> for Error {
    fn from(error: TryFromIntError) -> Self {
        Error::ConversionError(error)
    }
}

impl From<nb::Error<()>> for Error {
    fn from(_: nb::Error<()>) -> Self {
        Error::UnexpectedlyBlocks
    }
}
