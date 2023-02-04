use crate::board::{AudioEnable, Storage};
use crate::error::Error;
use core::cell::RefCell;
use littlefs2::fs::Filesystem;
use littlefs2::path::Path;
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

const STARTUP_CLIPS: &[&str] = &["Turret_sfx_deploy.raw\0", "Turret_sfx_active.raw\0"];
const BEGIN_SCAN_CLIPS: &[&str] = &[
    "Turret_searching.raw\0",
    "Turret_activated.raw\0",
    "Turret_sentry_mode_activated.raw\0",
    "Turret_could_you_come_over_here.raw\0",
    "Turret_deploying.raw\0",
];
const TARGET_ACQUIRED_CLIPS: &[&str] = &[
    "Turret_hello_friend.raw\0",
    "Turret_who_is_there.raw\0",
    "Turret_target_acquired.raw\0",
    "Turret_gotcha.raw\0",
    "Turret_I_see_you.raw\0",
    "Turret_there_you_are.raw\0",
];
const CONTACT_LOST_CLIPS: &[&str] = &["Turret_sfx_retract.raw\0"];
const CONTACT_RESTORED_CLIPS: &[&str] = &[
    "Turret_sfx_ping.raw\0",
    "Turret_hi.raw\0",
    "Turret_sfx_alert.raw\0",
];
const TARGET_LOST_CLIPS: &[&str] = &[
    "Turret_is_anyone_there.raw\0",
    "Turret_hellooooo.raw\0",
    "Turret_are_you_still_there.raw\0",
    "Turret_target_lost.raw\0",
];
const PICKED_UP_CLIPS: &[&str] = &[
    "Turret_malfunctioning.raw\0",
    "Turret_put_me_down.raw\0",
    "Turret_who_are_you.raw\0",
    "Turret_please_put_me_down.raw\0",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurrentlyPlaying {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
enum PlayState {
    Idle,
    Playing {
        path: &'static str,
        offset: usize,
        current: CurrentlyPlaying,
        next_buffer_size: usize,
        buf1: [u8; BUF_SIZE],
        buf2: [u8; BUF_SIZE],
    },
}

struct State {
    storage: Storage,
    audio_enable: AudioEnable,
    play_state: PlayState,
}

impl State {
    fn init(storage: Storage, audio_enable: AudioEnable) -> Result<Self, Error> {
        Ok(State {
            storage,
            audio_enable,
            play_state: PlayState::Idle,
        })
    }

    fn pick_clip(&self, clips: &[&'static str]) -> &'static str {
        // TODO select randomly
        clips[0]
    }

    fn play(&mut self, sound: Sound) -> Result<(), Error> {
        if self.play_state == PlayState::Idle {
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
        let filename = self.pick_clip(clips);

        rprintln!("playing {:?}", filename);

        self.play_state = PlayState::Playing {
            path: filename,
            offset: 0,
            current: CurrentlyPlaying::First,
            next_buffer_size: 0,
            buf1: [0; BUF_SIZE],
            buf2: [0; BUF_SIZE],
        };

        // Read first block into buffer and start playing it.
        Filesystem::mount_and_then(&mut self.storage, |fs| {
            fs.open_file_and_then(Path::from_bytes_with_nul(filename.as_bytes())?, |f| {
                match &mut self.play_state {
                    PlayState::Playing {
                        offset,
                        buf1,
                        next_buffer_size,
                        ..
                    } => {
                        let bytes_read = f.read(buf1)?;
                        *offset += bytes_read;
                        *next_buffer_size = bytes_read;

                        if *next_buffer_size > 0 {
                            // TODO start playing first buffer
                        } else {
                            // Empty clip, don't play
                            self.play_state = PlayState::Idle;
                        }
                        Ok(())
                    }
                    _ => unreachable!(),
                }
            })
        })
        .map_err(|err| {
            // First buffer read failed, bail out.
            self.play_state = PlayState::Idle;
            err
        })?;

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
