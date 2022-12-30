use core::fmt::{self, Debug, Display};

/// The error type used by this library.
///
/// This can encapsulate an SPI or GPIO error, and adds its own protocol errors
/// on top of that.
pub enum Error<SpiError, GpioError> {
    /// An SPI transfer failed.
    Spi(SpiError),

    /// A GPIO could not be set.
    Gpio(GpioError),

    /// Status register contained unexpected flags.
    ///
    /// This can happen when the chip is faulty, incorrectly connected, or the
    /// driver wasn't constructed or destructed properly (eg. while there is
    /// still a write in progress).
    UnexpectedStatus,
}

impl<SpiError, GpioError> Debug for Error<SpiError, GpioError>
where
    SpiError: Debug,
    GpioError: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spi(spi) => write!(f, "Error::Spi({:?})", spi),
            Error::Gpio(gpio) => write!(f, "Error::Gpio({:?})", gpio),
            Error::UnexpectedStatus => f.write_str("Error::UnexpectedStatus"),
        }
    }
}

impl<SpiError, GpioError> Display for Error<SpiError, GpioError>
where
    SpiError: Display,
    GpioError: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spi(spi) => write!(f, "SPI error: {}", spi),
            Error::Gpio(gpio) => write!(f, "GPIO error: {}", gpio),
            Error::UnexpectedStatus => f.write_str("unexpected value in status register"),
        }
    }
}
