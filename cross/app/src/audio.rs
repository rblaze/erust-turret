use crate::board::{AudioEnable, Storage};
use crate::error::Error;
use core::cell::RefCell;
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
    pub fn new(storage: Storage, audio_enable: AudioEnable) -> Result<Audio, Error> {
        STATE.set(State::init(storage, audio_enable)?);

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

enum PlayState {
    Idle,
    Playing {
        file: File<'static, Storage>,
        current_buffer: CurrentlyPlaying,
        bytes_in_next_buffer: usize,
    },
    LastBlock,
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
    fs: FileSystem<Storage>,
    audio_enable: AudioEnable,
    play_state: PlayState,
    buf1: [u8; BUF_SIZE],
    buf2: [u8; BUF_SIZE],
}

impl State {
    fn init(storage: Storage, audio_enable: AudioEnable) -> Result<Self, Error> {
        Ok(State {
            fs: FileSystem::mount(storage)?,
            audio_enable,
            play_state: PlayState::Idle,
            buf1: [0; BUF_SIZE],
            buf2: [0; BUF_SIZE],
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
        let clip = self.pick_clip(clips);

        rprintln!("playing {:?}", clip);

        let mut file = self.fs.open(clip.file_index())?;
        let bytes_read = file.read(&mut self.buf1)?;

        if bytes_read == 0 {
            rprintln!("Clip data is empty");
            return Ok(());
        }

        if bytes_read != BUF_SIZE {
            self.play_state = PlayState::LastBlock;
        } else {
            self.play_state = PlayState::Playing {
                // Filesystem is never unmounted, so it is safe to get static reference.
                file: unsafe { core::mem::transmute(file) },
                current_buffer: CurrentlyPlaying::First,
                bytes_in_next_buffer: 0,
            };
        }

        {
            self.start_playback()?;
            self.play_next_buffer()?;
            Ok(())
        }
        .or_else(|err: Error| {
            rprintln!("Error while starting sound: {:?}", err);

            self.end_playback();

            Err(err)
        })?;

        Ok(())
    }

    fn start_playback(&mut self) -> Result<(), Error> {
        // Init sound hardware
        self.audio_enable.set_high();

        Ok(())
    }

    fn play_next_buffer(&mut self) -> Result<(), Error> {
        match &mut self.play_state {
            PlayState::Idle => {
                debug_assert!(!self.play_state.is_idle());
                rprintln!("play_next_block called in Idle state");
            }
            PlayState::Playing {
                file,
                current_buffer,
                bytes_in_next_buffer,
            } => {
                todo!()
                // Start playing next buffer
                // Read more data
            }
            PlayState::LastBlock => {
                self.end_playback();
            }
        }

        Ok(())
    }

    fn end_playback(&mut self) {
        debug_assert!(self.play_state.is_idle());

        // Disable sound hardware
        self.audio_enable.set_low();
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
