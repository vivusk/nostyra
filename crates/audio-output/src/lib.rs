//! Audio output: SDL3 `AudioStream` fed by the core, with dynamic rate control,
//! plus the sync-mode decision (vsync vs audio-sync) that drives loop timing, and a
//! separate sound-effect stream (achievement chime).

#![deny(unsafe_code)]

mod dynamic_rate_control;
mod sdl_audio_stream_output;
mod sound_effect_stream;
mod sync_mode_selection;

pub use dynamic_rate_control::{
    audio_sync_ratio, drc_ratio, vsync_base_ratio, DEFAULT_MAX_DEVIATION,
};
pub use sdl_audio_stream_output::{AudioOutputError, AudioStats, SdlAudioOutput};
pub use sound_effect_stream::{SoundEffectStream, SOUND_EFFECT_RATE};
pub use sync_mode_selection::{select_sync_mode, SyncMode};
