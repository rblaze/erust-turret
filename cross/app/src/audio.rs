use crate::board::{AudioEnable, AudioPwm, Storage};
use crate::error::Error;
use crate::event_queue::{Event, EventQueue};
use core::cell::RefCell;
use rtt_target::rprintln;
use simplefs::{File, FileSystem};
use stm32f1xx_hal::pac::interrupt;
use stm32f1xx_hal::timer::Channel;

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
    pub fn new(
        event_queue: &mut EventQueue<'_, 'static>,
        storage: Storage,
        audio_enable: AudioEnable,
        audio_pwm: AudioPwm,
    ) -> Result<Audio, Error> {
        STATE.set(State::init(storage, audio_enable, audio_pwm)?);
        event_queue.bind(&PLAY_NEXT_BUFFER);

        Ok(Audio {})
    }

    pub fn play(&self, sound: Sound) {
        STATE.with(|state| state.play(sound)).unwrap();
    }
}

#[allow(dead_code)]
// Clips are unsigned 8 bit, 16 KHz.
pub const SOUND_FREQ: u32 = 16000;

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
    AreYouStillThere,
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
    Clip::AreYouStillThere,
    Clip::TargetLost,
];
const PICKED_UP_CLIPS: &[Clip] = &[
    Clip::Malfunctioning,
    Clip::PutMeDown,
    Clip::WhoAreYou,
    Clip::PleasePutMeDown,
];

enum PlayState {
    Idle,
    Playing {
        file: File<'static, Storage>,
        next_buffer_index: usize,
        bytes_in_next_buffer: usize,
    },
    LastBlock,
}

struct State {
    fs: FileSystem<Storage>,
    audio_enable: AudioEnable,
    audio_pwm: AudioPwm,
    play_state: PlayState,
    buffers: [[u8; BUF_SIZE]; 2],
}

impl State {
    fn init(
        storage: Storage,
        audio_enable: AudioEnable,
        audio_pwm: AudioPwm,
    ) -> Result<Self, Error> {
        Ok(State {
            fs: FileSystem::mount(storage)?,
            audio_enable,
            audio_pwm,
            play_state: PlayState::Idle,
            buffers: [[0; BUF_SIZE]; 2],
        })
    }

    fn pick_clip(&self, clips: &[Clip]) -> Clip {
        // TODO select randomly
        clips[0]
    }

    fn play(&mut self, sound: Sound) -> Result<(), Error> {
        if !matches!(self.play_state, PlayState::Idle) {
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
        let bytes_read = file.read(&mut self.buffers[0])?;

        if bytes_read == 0 {
            rprintln!("Clip data is empty");
            return Ok(());
        }

        self.play_state = PlayState::Playing {
            // Filesystem is never unmounted, so it is safe to get static reference.
            file: unsafe { core::mem::transmute(file) },
            next_buffer_index: 0,
            bytes_in_next_buffer: bytes_read,
        };

        {
            self.start_playback()?;
            self.play_next_buffer()
        }
        .map_err(|err| {
            rprintln!("Error while starting sound: {:?}", err);
            self.end_playback();

            err
        })?;

        Ok(())
    }

    fn play_next_buffer(&mut self) -> Result<(), Error> {
        let state = &mut self.play_state;
        match state {
            PlayState::Idle => {
                debug_assert!(!matches!(self.play_state, PlayState::Idle));
                rprintln!("play_next_block called in Idle state");
            }
            PlayState::Playing {
                file,
                next_buffer_index,
                bytes_in_next_buffer,
            } => {
                let play_buffer_index = *next_buffer_index;
                *next_buffer_index = (play_buffer_index + 1) % 2;

                // Start playing next buffer
                Self::play_buffer(&self.buffers[play_buffer_index])?;

                // Read more data
                *bytes_in_next_buffer = file.read(&mut self.buffers[*next_buffer_index])?;
                if *bytes_in_next_buffer == 0 {
                    self.play_state = PlayState::LastBlock;
                }
            }
            PlayState::LastBlock => {
                self.end_playback();
            }
        }

        Ok(())
    }

    fn start_playback(&mut self) -> Result<(), Error> {
        // TODO: enable TIM2 and DMA
        self.audio_enable.set_high();
        self.audio_pwm.enable(Channel::C2);

        Ok(())
    }

    fn play_buffer(_buffer: &[u8]) -> Result<(), Error> {
        // TODO: trigger DMA

        // FIXME: until DMA is here, schedule next iteration immediatelly.
        rprintln!("Playing buffer");
        PLAY_NEXT_BUFFER.call();
        Ok(())
    }

    fn end_playback(&mut self) {
        debug_assert!(!matches!(self.play_state, PlayState::Idle));

        self.audio_enable.set_low();
        self.audio_pwm.disable(Channel::C2);
        self.audio_pwm.set_duty(Channel::C2, 0);
        // TODO: disable TIM2 and DMA

        self.play_state = PlayState::Idle;
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

static PLAY_NEXT_BUFFER: Event =
    Event::new(&|| STATE.with(|state| state.play_next_buffer()).unwrap());

#[interrupt]
fn DMA1_CHANNEL5() {
    PLAY_NEXT_BUFFER.call();
}
