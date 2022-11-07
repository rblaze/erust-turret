#![cfg_attr(not(test), no_std)]
#![deny(unsafe_code)]

mod default_config;
mod registers;

use core::fmt::{Display, Formatter};

use crate::default_config::VL51L1X_DEFAULT_CONFIGURATION;
use crate::registers::*;
use embedded_hal::blocking::i2c::{Write, WriteRead};

pub const ADDR: u8 = 0x29;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VL53L1X<I2C> {
    i2c: I2C,
    addr: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Error<E> {
    I2C(E),
    InvalidTimingForDistanceMode,
    InvalidTimingBudget,
    InvalidDistanceMode,
}

impl<E> From<E> for Error<E> {
    fn from(i2c_error: E) -> Self {
        Error::I2C(i2c_error)
    }
}

impl<E: Display + Copy> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Error::I2C(error) => error.fmt(f),
            Error::InvalidTimingForDistanceMode => {
                f.pad("timing unsupported for set distance mode")
            }
            Error::InvalidTimingBudget => f.pad("invalid timing budget"),
            Error::InvalidDistanceMode => f.pad("invalid distance mode"),
        }
    }
}

// Magic numbers and comments below are from the STM driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BootState {
    Booted,
    NotBooted,
}

impl Display for BootState {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(match *self {
            BootState::Booted => "booted",
            BootState::NotBooted => "not booted",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DistanceMode {
    Short,
    Long,
}

impl Display for DistanceMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(match *self {
            DistanceMode::Short => "short",
            DistanceMode::Long => "long",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TimingBudget {
    Ms15, /* only available in short distance mode */
    Ms20,
    Ms33,
    Ms50,
    Ms100,
    Ms200,
    Ms500,
}

impl Display for TimingBudget {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.pad(match *self {
            TimingBudget::Ms15 => "15 ms",
            TimingBudget::Ms20 => "20 ms",
            TimingBudget::Ms33 => "33 ms",
            TimingBudget::Ms50 => "50 ms",
            TimingBudget::Ms100 => "100 ms",
            TimingBudget::Ms200 => "200 ms",
            TimingBudget::Ms500 => "500 ms",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RangeStatus {
    /// No error
    Ok,
    /// When the range status is 1, there is a sigma failure. This means
    /// that the repeatability or standard deviation of the measurement is
    /// bad due to a decreasing signal noise ratio. Increasing the timing
    /// budget can improve the standard deviation and avoid a range status 1.
    SigmaFailureWarning,
    /// When the range status is 2, there is a signal failure. This means
    /// that the return signal is too week to return a good answer. The
    /// reason is because the target is too far, or the target is not
    /// reflective enough, or the target is too small. Increasing the timing
    /// buget might help, but there may simply be no target available.
    SignalFailureWarning,
    /// When the range status is 4, the sensor is "out of bounds". This
    /// mean that the sensor is ranging in a “non-appropriated” zone and the
    /// measured result may be inconsistent. This status is considered as a
    /// warning but, in general, it happens when a target is at the maximum
    /// distance possible from the sensor, i.e. around 5 m. However, this is
    /// only for very bright targets.
    OutOfBoundsError,
    /// Range status 7 is called "wraparound". This situation may occur when
    /// the target is very reflective and the distance to the target/sensor
    /// is longer than the physical limited distance measurable by the sensor.
    /// Such distances include approximately 5 m when the senor is in Long
    /// distance mode and approximately 1.3 m when the sensor is in Short
    /// distance mode. Example: a traffic sign located at 6 m can be seen by
    /// the sensor and returns a range of 1 m. This is due to “radar
    /// aliasing”: if only an approximate distance is required, we may add
    /// 6 m to the distance returned. However, that is a very approximate
    /// estimation.
    WraparoundError,
    /// Error code not documented by STM.
    Undocumented(u8),
    /// Register value doesn't map to error code.
    InvalidRegisterValue(u8),
}

impl Display for RangeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            RangeStatus::Ok => f.pad("Ok"),
            RangeStatus::SigmaFailureWarning => f.pad("sigma failure"),
            RangeStatus::SignalFailureWarning => f.pad("signal failure"),
            RangeStatus::OutOfBoundsError => f.pad("out of bounds"),
            RangeStatus::WraparoundError => f.pad("wraparound"),
            RangeStatus::Undocumented(status) => {
                f.write_fmt(format_args!("undocumented({})", status))
            }
            RangeStatus::InvalidRegisterValue(status) => {
                f.write_fmt(format_args!("invalid({})", status))
            }
        }
    }
}

impl<I2C, E> VL53L1X<I2C>
where
    I2C: WriteRead<Error = E> + Write<Error = E>,
{
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self { i2c, addr }
    }

    /// Returns the Boot state of the device.
    pub fn boot_state(&mut self) -> Result<BootState, Error<E>> {
        let mut buffer = [0];
        self.i2c.write_read(
            self.addr,
            &VL53L1_FIRMWARE__SYSTEM_STATUS.to_be_bytes(),
            &mut buffer,
        )?;

        Ok(if buffer[0] == 0 {
            BootState::NotBooted
        } else {
            BootState::Booted
        })
    }

    /// Loads the 45 (sic, actually 91) bytes of configuration values which
    /// initialize the sensor.
    pub fn sensor_init(&mut self) -> Result<(), Error<E>> {
        for reg in 0x2d..0x88u16 {
            let byte_offset = (reg - 0x2d) as usize;

            self.write_u8(reg, VL51L1X_DEFAULT_CONFIGURATION[byte_offset])?;
        }

        self.start_ranging()?;
        while !(self.check_for_data_ready()?) {
            // busy-wait
        }
        self.clear_interrupt()?;
        self.stop_ranging()?;

        self.write_u8(VL53L1_VHV_CONFIG__TIMEOUT_MACROP_LOOP_BOUND, 0x09)?; /* two bounds VHV */
        self.write_u8(0x0b, 0)?; /* start VHV from the previous temperature */

        Ok(())
    }

    /// Returns the current timing budget in ms.
    pub fn get_timing_budget(&mut self) -> Result<TimingBudget, Error<E>> {
        let mut buffer = [0, 0];

        self.i2c.write_read(
            self.addr,
            &RANGE_CONFIG__TIMEOUT_MACROP_A_HI.to_be_bytes(),
            &mut buffer,
        )?;

        match u16::from_be_bytes(buffer) {
            0x001d => Ok(TimingBudget::Ms15),
            0x0051 => Ok(TimingBudget::Ms20),
            0x001e => Ok(TimingBudget::Ms20),
            0x00d6 => Ok(TimingBudget::Ms33),
            0x0060 => Ok(TimingBudget::Ms33),
            0x01ae => Ok(TimingBudget::Ms50),
            0x00ad => Ok(TimingBudget::Ms50),
            0x02e1 => Ok(TimingBudget::Ms100),
            0x01cc => Ok(TimingBudget::Ms100),
            0x03e1 => Ok(TimingBudget::Ms200),
            0x02d9 => Ok(TimingBudget::Ms200),
            0x0591 => Ok(TimingBudget::Ms500),
            0x048f => Ok(TimingBudget::Ms500),
            _ => Err(Error::InvalidTimingBudget),
        }
    }

    /// Programs the timing budget in ms. The predefined values are 15, 20,
    /// 50, 100, 200, and 500.
    /// This function must be called after the set_distance_mode.
    pub fn set_timing_budget(&mut self, budget: TimingBudget) -> Result<(), Error<E>> {
        let mode = self.get_distance_mode()?;

        let (a_hi, b_hi): (u16, u16) = match mode {
            DistanceMode::Short => match budget {
                TimingBudget::Ms15 => Ok((0x001d, 0x0027)),
                TimingBudget::Ms20 => Ok((0x0051, 0x006e)),
                TimingBudget::Ms33 => Ok((0x00d6, 0x006e)),
                TimingBudget::Ms50 => Ok((0x01ae, 0x01e8)),
                TimingBudget::Ms100 => Ok((0x02e1, 0x0388)),
                TimingBudget::Ms200 => Ok((0x03e1, 0x0496)),
                TimingBudget::Ms500 => Ok((0x0591, 0x05c1)),
            },
            DistanceMode::Long => match budget {
                TimingBudget::Ms15 => Err(Error::InvalidTimingForDistanceMode),
                TimingBudget::Ms20 => Ok((0x001e, 0x0022)),
                TimingBudget::Ms33 => Ok((0x0060, 0x006e)),
                TimingBudget::Ms50 => Ok((0x00ad, 0x00c6)),
                TimingBudget::Ms100 => Ok((0x01cc, 0x01ea)),
                TimingBudget::Ms200 => Ok((0x02d9, 0x02f8)),
                TimingBudget::Ms500 => Ok((0x048f, 0x04a4)),
            },
        }?;

        self.write_u16(RANGE_CONFIG__TIMEOUT_MACROP_A_HI, a_hi)?;
        self.write_u16(RANGE_CONFIG__TIMEOUT_MACROP_B_HI, b_hi)?;

        Ok(())
    }

    /// Programs the intermeasurement period (IMP) in ms. The IMP must be
    /// greater than or equal to the timing budget. This condition is not
    /// checked by the API, so the customer must check this condition.
    pub fn set_inter_measurement(
        &mut self,
        period: fugit::MillisDurationU32,
    ) -> Result<(), Error<E>> {
        let mut buffer = [0, 0];
        self.i2c.write_read(
            self.addr,
            &VL53L1_RESULT__OSC_CALIBRATE_VAL.to_be_bytes(),
            &mut buffer,
        )?;

        let clock_pll: u32 = u16::from_be_bytes(buffer) as u32 & 0x3ff;
        let ticks = clock_pll * period.ticks() * 43 / 40; // 43/40 = 1.075, as in STM driver

        self.write_u32(VL53L1_SYSTEM__INTERMEASUREMENT_PERIOD, ticks)?;

        Ok(())
    }

    /// Returns the current distance mode.
    pub fn get_distance_mode(&mut self) -> Result<DistanceMode, Error<E>> {
        let mut buffer = [0];

        self.i2c.write_read(
            self.addr,
            &PHASECAL_CONFIG__TIMEOUT_MACROP.to_be_bytes(),
            &mut buffer,
        )?;

        match buffer[0] {
            0x14 => Ok(DistanceMode::Short),
            0x0a => Ok(DistanceMode::Long),
            _ => Err(Error::InvalidDistanceMode),
        }
    }

    /// Programs the distance mode (1 = Short, 2 = Long). Short mode maximum
    /// distance is limited to 1.3 m but results in a better ambient immunity.
    /// Long mode can range up to 4 m in the dark with a timing budget of 200 ms.
    pub fn set_distance_mode(&mut self, mode: DistanceMode) -> Result<(), Error<E>> {
        let timing_budget = self.get_timing_budget()?;

        // Here be dragons
        match mode {
            DistanceMode::Short => {
                self.write_u8(PHASECAL_CONFIG__TIMEOUT_MACROP, 0x14)?;
                self.write_u8(RANGE_CONFIG__VCSEL_PERIOD_A, 0x07)?;
                self.write_u8(RANGE_CONFIG__VCSEL_PERIOD_B, 0x05)?;
                self.write_u8(RANGE_CONFIG__VALID_PHASE_HIGH, 0x38)?;
                self.write_u16(SD_CONFIG__WOI_SD0, 0x0705)?;
                self.write_u16(SD_CONFIG__INITIAL_PHASE_SD0, 0x0606)?;
            }
            DistanceMode::Long => {
                self.write_u8(PHASECAL_CONFIG__TIMEOUT_MACROP, 0x0a)?;
                self.write_u8(RANGE_CONFIG__VCSEL_PERIOD_A, 0x0f)?;
                self.write_u8(RANGE_CONFIG__VCSEL_PERIOD_B, 0x0d)?;
                self.write_u8(RANGE_CONFIG__VALID_PHASE_HIGH, 0xb8)?;
                self.write_u16(SD_CONFIG__WOI_SD0, 0x0f0d)?;
                self.write_u16(SD_CONFIG__INITIAL_PHASE_SD0, 0x0e0e)?;
            }
        }

        // Restore timing budget and hope it works with the distance mode
        self.set_timing_budget(timing_budget)
    }

    /// Starts the ranging distance operation which is continuous. The clear
    /// interrupt has to be done after each "get data" to allow the interrupt
    /// to be raised when the next data are ready.
    pub fn start_ranging(&mut self) -> Result<(), Error<E>> {
        self.write_u8(SYSTEM__MODE_START, 0x40)?; /* Enable VL53L1X */
        Ok(())
    }

    /// Stops the ranging.
    pub fn stop_ranging(&mut self) -> Result<(), Error<E>> {
        self.write_u8(SYSTEM__MODE_START, 0x00)?; /* Disable VL53L1X */
        Ok(())
    }

    /// Checks if the new ranging data are available by polling the dedicated
    /// register.
    pub fn check_for_data_ready(&mut self) -> Result<bool, Error<E>> {
        let polarity = self.get_interrupt_polarity()?;

        let mut buffer = [0];
        self.i2c
            .write_read(self.addr, &GPIO__TIO_HV_STATUS.to_be_bytes(), &mut buffer)?;

        Ok(buffer[0] & 0x01 == polarity)
    }

    /// Returns the distance measured by the sensor in mm.
    pub fn get_distance(&mut self) -> Result<u16, Error<E>> {
        let mut buffer = [0; 2];

        self.i2c.write_read(
            self.addr,
            &VL53L1_RESULT__FINAL_CROSSTALK_CORRECTED_RANGE_MM_SD0.to_be_bytes(),
            &mut buffer,
        )?;

        Ok(u16::from_be_bytes(buffer))
    }

    /// Returns the ranging status error.
    pub fn get_range_status(&mut self) -> Result<RangeStatus, Error<E>> {
        let mut buffer = [0];

        self.i2c.write_read(
            self.addr,
            &VL53L1_RESULT__RANGE_STATUS.to_be_bytes(),
            &mut buffer,
        )?;

        let status = buffer[0] & 0x1f;
        Ok(match status {
            3 => RangeStatus::Undocumented(5),
            4 => RangeStatus::SignalFailureWarning,
            5 => RangeStatus::OutOfBoundsError,
            6 => RangeStatus::SigmaFailureWarning,
            7 => RangeStatus::WraparoundError,
            8 => RangeStatus::Undocumented(3),
            9 => RangeStatus::Ok,
            12 => RangeStatus::Undocumented(9),
            13 => RangeStatus::Undocumented(13),
            18 => RangeStatus::Undocumented(10),
            19 => RangeStatus::Undocumented(6),
            22 => RangeStatus::Undocumented(11),
            23 => RangeStatus::Undocumented(12),
            v => RangeStatus::InvalidRegisterValue(v),
        })
    }

    /// Clears the interrupt to be called after a ranging data reading, to arm
    /// the interrupt for the next data ready event.
    pub fn clear_interrupt(&mut self) -> Result<(), Error<E>> {
        self.write_u8(SYSTEM__INTERRUPT_CLEAR, 1)?;
        Ok(())
    }

    /// Returns the current interrupt polarity.
    /// 1 = active high (default), 0 = active low.
    pub fn get_interrupt_polarity(&mut self) -> Result<u8, Error<E>> {
        let mut buffer = [0];
        self.i2c
            .write_read(self.addr, &GPIO_HV_MUX__CTRL.to_be_bytes(), &mut buffer)?;
        // Get bit under mask 0x10 and invert it, return as 1 or 0
        Ok(((buffer[0] & 0x10) >> 4) ^ 1)
    }

    fn write_u8(&mut self, register: Register, data: u8) -> Result<(), Error<E>> {
        let reg_bytes = register.to_be_bytes();
        self.i2c
            .write(self.addr, &[reg_bytes[0], reg_bytes[1], data])?;

        Ok(())
    }

    fn write_u16(&mut self, register: Register, data: u16) -> Result<(), Error<E>> {
        let reg_bytes = register.to_be_bytes();
        let data_bytes = data.to_be_bytes();
        self.i2c.write(
            self.addr,
            &[reg_bytes[0], reg_bytes[1], data_bytes[0], data_bytes[1]],
        )?;

        Ok(())
    }

    fn write_u32(&mut self, register: Register, data: u32) -> Result<(), Error<E>> {
        let reg_bytes = register.to_be_bytes();
        let data_bytes = data.to_be_bytes();
        self.i2c.write(
            self.addr,
            &[
                reg_bytes[0],
                reg_bytes[1],
                data_bytes[0],
                data_bytes[1],
                data_bytes[2],
                data_bytes[3],
            ],
        )?;

        Ok(())
    }
}
