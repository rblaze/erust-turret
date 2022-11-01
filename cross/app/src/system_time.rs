#![deny(unsafe_code)]

use core::cell::Cell;
use cortex_m::peripheral::SYST;
use cortex_m_rt::exception;
use critical_section::Mutex;

pub type Instant = fugit::TimerInstantU32<100>;
pub type Duration = fugit::TimerDurationU32<100>;

static TICKS: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

pub struct Ticker {
    syst: SYST,
}

impl Ticker {
    // Setup SysTick to tick at 100Hz
    pub fn new(mut syst: SYST) -> Self {
        let reload = SYST::get_ticks_per_10ms();
        assert_ne!(reload, 0);

        syst.set_reload(reload);
        syst.clear_current();
        syst.enable_interrupt();
        syst.enable_counter();

        Ticker { syst }
    }

    // Stop SysTick and release it
    #[allow(dead_code)]
    pub fn free(mut self) -> SYST {
        self.syst.disable_interrupt();
        self.syst.disable_counter();
        self.syst
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
