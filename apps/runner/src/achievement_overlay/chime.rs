//! The unlock chime: a 300 ms rising two-note sine (A5 then E6, 150 ms each, 5 ms
//! attack and an exponential decay) synthesized once in code (no audio asset), played
//! on its own sound-effect stream at a fixed volume (user decision). Silent when the
//! `sound` setting is off and during fast-forward.

use std::f64::consts::TAU;

use audio_output::{SoundEffectStream, SOUND_EFFECT_RATE};
use sdl3::AudioSubsystem;

/// Fixed gain (well below the core's full scale).
const GAIN: f64 = 0.3;
const NOTES_HZ: [f64; 2] = [880.0, 1318.5];
const NOTE_MS: usize = 150;

pub struct AchievementChime {
    stream: SoundEffectStream,
    samples: Vec<f32>,
}

impl AchievementChime {
    /// Opens the effect stream; `None` (logged) when the device refuses it.
    pub fn open(audio: &AudioSubsystem) -> Option<Self> {
        match SoundEffectStream::open(audio) {
            Ok(stream) => Some(Self {
                stream,
                samples: chime_samples(SOUND_EFFECT_RATE as u32),
            }),
            Err(err) => {
                tracing::warn!(%err, "achievement chime unavailable");
                None
            }
        }
    }

    pub fn play(&self) {
        if let Err(err) = self.stream.play(&self.samples) {
            tracing::warn!(%err, "achievement chime failed");
        }
    }
}

/// Whether a popup that just appeared rings: it chimes, sound is on, no fast-forward.
pub fn should_chime(popup_chimes: bool, sound: bool, fast_forward: bool) -> bool {
    popup_chimes && sound && !fast_forward
}

/// Mono f32 samples of the chime at `rate`.
pub fn chime_samples(rate: u32) -> Vec<f32> {
    let note = rate as usize * NOTE_MS / 1000;
    NOTES_HZ
        .iter()
        .flat_map(|&freq| {
            (0..note).map(move |i| {
                let t = i as f64 / f64::from(rate);
                let envelope = (t / 0.005).min(1.0) * (-t * 18.0).exp();
                ((TAU * freq * t).sin() * envelope * GAIN) as f32
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chime_is_300_ms_quiet_and_click_free_at_the_edges() {
        let samples = chime_samples(48_000);
        assert_eq!(samples.len(), 14_400, "300 ms at 48 kHz");
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.1 && peak <= GAIN as f32, "peak {peak}");
        assert_eq!(samples[0], 0.0, "starts from silence");
        assert!(
            samples[7_199].abs() < 0.05,
            "first note decayed before the second"
        );
        assert!(samples.last().unwrap().abs() < 0.05);
    }

    #[test]
    fn mute_rules() {
        assert!(should_chime(true, true, false));
        assert!(!should_chime(true, false, false), "sound setting off");
        assert!(!should_chime(true, true, true), "fast-forward");
        assert!(!should_chime(false, true, false), "placard / notice");
    }
}
