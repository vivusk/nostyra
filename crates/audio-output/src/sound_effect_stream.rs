//! A short sound effect (the achievement unlock chime) next to the core audio. It
//! opens its **own** logical device on the default playback device
//! (`SDL_OpenAudioDeviceStream`): the core stream's device is SDL's simplified kind,
//! which refuses other bindings, and wrapping its id in an `AudioDevice` would close
//! core audio on drop (M15 spike D10). SDL mixes the two logical devices itself, so
//! the effect is independent of the core stream's pause, rate control and reopen.

use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::AudioSubsystem;

use crate::AudioOutputError;

/// Mono f32 sample rate of effect buffers (SDL converts to the device format).
pub const SOUND_EFFECT_RATE: i32 = 48_000;

/// One playing slot: a new effect replaces the one still sounding.
pub struct SoundEffectStream {
    stream: AudioStreamOwner,
}

impl SoundEffectStream {
    /// Opens the effect device (silent until [`Self::play`]).
    pub fn open(audio: &AudioSubsystem) -> Result<Self, AudioOutputError> {
        let spec = AudioSpec {
            freq: Some(SOUND_EFFECT_RATE),
            channels: Some(1),
            format: Some(AudioFormat::f32_sys()),
        };
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&spec))?;
        stream.resume()?;
        Ok(Self { stream })
    }

    /// Plays mono `samples` at [`SOUND_EFFECT_RATE`] from now (cuts a previous one).
    pub fn play(&self, samples: &[f32]) -> Result<(), AudioOutputError> {
        self.stream.clear()?;
        self.stream.put_data_f32(samples)?;
        self.stream.flush()?;
        Ok(())
    }
}
