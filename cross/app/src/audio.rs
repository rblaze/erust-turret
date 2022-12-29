use rtt_target::rprintln;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Startup,
    BeginScan,
    TargetAcquired,
    ContactLost,
    ContactRestored,
    TargetLost,
    #[allow(dead_code)]
    PickedUp, // Sensor not on board
}

#[derive(Clone, Copy)]
pub struct Audio;

impl Audio {
    pub fn new() -> Audio {
        Audio {}
    }

    pub fn play(&self, sound: Sound) {
        rprintln!("playing {:?}", sound);
    }
}

#[allow(dead_code)]
// Clips are unsigned 8 bit, 16 KHz.
const SOUND_FREQ: u16 = 16000;

#[allow(dead_code)]
// Sound buffer size.
const BUF_SIZE: usize = 1024;
