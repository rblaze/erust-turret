#![deny(unsafe_code)]

use core::cell::Cell;
use cortex_m_rt::exception;
use critical_section::Mutex;
use fugit::RateExtU32;
use stm32f1xx_hal::pac::SYST;
use stm32f1xx_hal::timer::{SysCounterHz, SysEvent, Timer};

const HERTZ: u32 = 100;

pub type Instant = fugit::TimerInstantU32<HERTZ>;
pub type Duration = fugit::TimerDurationU32<HERTZ>;

static TICKS: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

pub struct Ticker {
    _counter: SysCounterHz,
}

impl Ticker {
    // Setup SysTick to tick at 100Hz
    pub fn new(syst: Timer<SYST>) -> Self {
        let mut counter = syst.counter_hz();

        counter.start(HERTZ.Hz()).unwrap();
        counter.listen(SysEvent::Update);

        Ticker { _counter: counter }
    }

    // Get current tick count
    pub fn get_ticks(&self) -> u32 {
        critical_section::with(|cs| TICKS.borrow(cs).get())
    }

    // Get timestamp
    #[allow(dead_code)]
    pub fn get(&self) -> Instant {
        let ticks = self.get_ticks();
        Instant::from_ticks(ticks)
    }
}

#[exception]
fn SysTick() {
    critical_section::with(|cs| {
        let ticks = TICKS.borrow(cs).get();
        TICKS.borrow(cs).set(ticks + 1);
    });
}
