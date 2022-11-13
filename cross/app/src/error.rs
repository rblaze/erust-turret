use core::num::TryFromIntError;

#[derive(Debug)]
pub enum Error {
    Sensor(vl53l1x::Error<nb::Error<stm32f1xx_hal::i2c::Error>>),
    InvalidDuration,
    ConversionError(TryFromIntError),
    UnexpectedlyBlocks,
}

impl From<vl53l1x::Error<nb::Error<stm32f1xx_hal::i2c::Error>>> for Error {
    fn from(sensor_error: vl53l1x::Error<nb::Error<stm32f1xx_hal::i2c::Error>>) -> Self {
        Error::Sensor(sensor_error)
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
