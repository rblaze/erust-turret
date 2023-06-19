#![no_std]
#![deny(unsafe_code)]

use stm32f1xx_hal::device::I2C1;
use stm32f1xx_hal::gpio::{Alternate, Input, Output};
use stm32f1xx_hal::gpio::{Floating, OpenDrain, PullDown, PushPull};
use stm32f1xx_hal::gpio::{PA4, PA5, PA8, PA9, PB12, PB13, PB14, PB15, PB3, PB5, PB6, PB7};
use stm32f1xx_hal::i2c::BlockingI2c;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::spi::{Spi, Spi2NoRemap};

pub type I2cScl = PB6<Alternate<OpenDrain>>;
pub type I2cSda = PB7<Alternate<OpenDrain>>;
pub type I2cBus = BlockingI2c<I2C1, (I2cScl, I2cSda)>;

pub type SensorServoPin = PA8<Alternate<PushPull>>;

pub type Laser = PA5<Output<PushPull>>;
pub type LaserServoPin = PA9<Alternate<PushPull>>;

pub type Led = PB3<Output<PushPull>>;
pub type Button = PB5<Input<PullDown>>;

pub type SpiCs = PB12<Output<PushPull>>;
pub type SpiClk = PB13<Alternate<PushPull>>;
pub type SpiMiso = PB14<Input<Floating>>;
pub type SpiMosi = PB15<Alternate<PushPull>>;
pub type SpiBus = Spi<pac::SPI2, Spi2NoRemap, (SpiClk, SpiMiso, SpiMosi), u8>;

pub type AudioEnable = PA4<Output<PushPull>>;
