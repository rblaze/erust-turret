#![deny(unsafe_code)]

use crate::error::Error;

// use rtt_target::rprintln;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::serial::Config;
use stm32f1xx_hal::spi::Spi;

pub use board::{Button, Led, SpiBus, SpiCs, Uart};
pub type SpiMemory = spi_memory::series25::Flash<SpiBus, SpiCs>;

pub struct Board {
    button: Button,
    led: Led,
    serial: Uart,
    memory: SpiMemory,
}

impl Board {
    pub fn new(cp: pac::CorePeripherals, dp: pac::Peripherals) -> Result<Self, Error> {
        // Enable debug while sleeping to keep probe-rs happy while WFI
        dp.DBGMCU.cr.modify(|_, w| {
            w.dbg_sleep().set_bit();
            w.dbg_standby().set_bit();
            w.dbg_stop().set_bit()
        });
        dp.RCC.ahbenr.modify(|_, w| w.dma1en().enabled());

        // Configure the clock.
        let mut flash = dp.FLASH.constrain();
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(64.MHz()).freeze(&mut flash.acr);

        let mut afio = dp.AFIO.constrain();

        // Acquire the GPIO peripherals.
        let mut gpioa = dp.GPIOA.split();
        let mut gpiob = dp.GPIOB.split();

        // Disable JTAG to get PB3 (mistake in board design)
        let (_, pb3, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);

        let led = pb3.into_push_pull_output(&mut gpiob.crl);
        let button = gpiob.pb5.into_pull_down_input(&mut gpiob.crl);

        let spi_cs = gpiob.pb12.into_push_pull_output(&mut gpiob.crh);
        let spi_clk = gpiob.pb13.into_alternate_push_pull(&mut gpiob.crh);
        let spi_miso = gpiob.pb14.into_floating_input(&mut gpiob.crh);
        let spi_mosi = gpiob.pb15.into_alternate_push_pull(&mut gpiob.crh);

        let spi = Spi::spi2(
            dp.SPI2,
            (spi_clk, spi_miso, spi_mosi),
            embedded_hal::spi::MODE_0,
            10.MHz(),
            clocks,
        );

        let memory = SpiMemory::init(spi, spi_cs)?;

        let serial_tx = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
        let serial_rx = gpioa.pa3.into_floating_input(&mut gpioa.crl);
        let serial = Uart::new(
            dp.USART2,
            (serial_tx, serial_rx),
            &mut afio.mapr,
            Config::default()
                .baudrate(115200.bps())
                .wordlength_8bits()
                .parity_none(),
            &clocks,
        );

        Ok(Board {
            button,
            led,
            serial,
            memory,
        })
    }
}
