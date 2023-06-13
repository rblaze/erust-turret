use crate::board::{AudioEnable, Storage};
use crate::error::Error;
use core::cell::RefCell;
use core::mem::transmute;
use rtt_target::rprintln;
use simplefs::{File, FileSystem};

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
    pub fn new(audio_enable: AudioEnable) -> Result<Audio, Error> {
        STATE.set(State::init(audio_enable)?);

        Ok(Audio {})
    }

    pub fn play(&self, sound: Sound) {
        STATE.with(|state| state.play(sound)).unwrap();
    }
}

#[allow(dead_code)]
// Clips are unsigned 8 bit, 16 KHz.
const SOUND_FREQ: u16 = 16000;

#[allow(dead_code)]
// Sound buffer size.
const BUF_SIZE: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clip {
    SfxDeploy,
    SfxActive,
    Searching,
    Activated,
    SentryModeActivated,
    CouldYouComeOverHere,
    Deploying,
    HelloFriend,
    WhoIsThere,
    TargetAcquired,
    Gotcha,
    ISeeYou,
    ThereYouAre,
    SfxRetract,
    SfxPing,
    Hi,
    SfxAlert,
    IsAnyoneThere,
    Hellooooo,
    IAmStillThere,
    TargetLost,
    Malfunctioning,
    PutMeDown,
    WhoAreYou,
    PleasePutMeDown,
}

impl Clip {
    const fn file_index(self) -> usize {
        self as usize
    }
}

const STARTUP_CLIPS: &[Clip] = &[Clip::SfxDeploy, Clip::SfxActive];
const BEGIN_SCAN_CLIPS: &[Clip] = &[
    Clip::Searching,
    Clip::Activated,
    Clip::SentryModeActivated,
    Clip::CouldYouComeOverHere,
    Clip::Deploying,
];
const TARGET_ACQUIRED_CLIPS: &[Clip] = &[
    Clip::HelloFriend,
    Clip::WhoIsThere,
    Clip::TargetAcquired,
    Clip::Gotcha,
    Clip::ISeeYou,
    Clip::ThereYouAre,
];
const CONTACT_LOST_CLIPS: &[Clip] = &[Clip::SfxRetract];
const CONTACT_RESTORED_CLIPS: &[Clip] = &[Clip::SfxPing, Clip::Hi, Clip::SfxAlert];
const TARGET_LOST_CLIPS: &[Clip] = &[
    Clip::IsAnyoneThere,
    Clip::Hellooooo,
    Clip::IAmStillThere,
    Clip::TargetLost,
];
const PICKED_UP_CLIPS: &[Clip] = &[
    Clip::Malfunctioning,
    Clip::PutMeDown,
    Clip::WhoAreYou,
    Clip::PleasePutMeDown,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurrentlyPlaying {
    First,
    Second,
}

#[allow(clippy::large_enum_variant)]
enum PlayState {
    Idle,
    Playing {
        file: File<'static, Storage>,
        current_buffer: CurrentlyPlaying,
        bytes_in_next_buffer: usize,
        buf1: [u8; BUF_SIZE],
        buf2: [u8; BUF_SIZE],
    },
}

impl PlayState {
    fn is_idle(&self) -> bool {
        match self {
            PlayState::Idle => true,
            _ => false,
        }
    }
}

struct State {
    // fs: &'static FileSystem<Storage>,
    audio_enable: AudioEnable,
    play_state: PlayState,
}

impl State {
    fn init(audio_enable: AudioEnable) -> Result<Self, Error> {
        Ok(State {
            // The filesystem is never unmounted unless the program panics anyway.
            // We can cast it to 'static lifetime.
            // fs: unsafe { transmute(fs) },
            audio_enable,
            play_state: PlayState::Idle,
        })
    }

    fn pick_clip(&self, clips: &[Clip]) -> Clip {
        // TODO select randomly
        clips[0]
    }

    fn play(&mut self, sound: Sound) -> Result<(), Error> {
        if !self.play_state.is_idle() {
            rprintln!("Audio busy");
            return Ok(());
        }

        let clips = match sound {
            Sound::Startup => STARTUP_CLIPS,
            Sound::BeginScan => BEGIN_SCAN_CLIPS,
            Sound::TargetAcquired => TARGET_ACQUIRED_CLIPS,
            Sound::ContactLost => CONTACT_LOST_CLIPS,
            Sound::ContactRestored => CONTACT_RESTORED_CLIPS,
            Sound::TargetLost => TARGET_LOST_CLIPS,
            Sound::PickedUp => PICKED_UP_CLIPS,
        };
        let file = self.pick_clip(clips);

        rprintln!("playing {:?}", file);

        // self.play_state = PlayState::Playing {
        //     file: ,
        //     current_buffer: CurrentlyPlaying::First,
        //     bytes_in_next_buffer: 0,
        //     buf1: [0; BUF_SIZE],
        //     buf2: [0; BUF_SIZE],
        // };

        Ok(())
    }
}

struct StaticState {
    state: RefCell<Option<State>>,
}

impl StaticState {
    const fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }

    fn set(&self, state: State) {
        *self.state.borrow_mut() = Some(state);
    }

    fn with<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: Fn(&mut State) -> Result<R, Error>,
    {
        let mut stref = self.state.borrow_mut();
        let state = stref.as_mut().ok_or(Error::Uninitialized)?;

        f(state)
    }
}

// STATE is only accessed from the main thread via EventQueue.
// Therefore, no locking is necessary.
unsafe impl Sync for StaticState {}

static STATE: StaticState = StaticState::new();
