//! Chooses what drives emulation timing for a given display (system-architecture §3.1).

use crate::dynamic_rate_control::{audio_sync_ratio, vsync_base_ratio};

/// Max |vsync base ratio − 1| for which vsync pacing is acceptable (1% pitch/speed).
const VSYNC_TOLERANCE: f64 = 0.01;

/// Timing master of the emulation loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SyncMode {
    /// One emulated frame per display refresh (Fifo); audio ratio absorbs the
    /// small fps mismatch. Smoothest on ~60 Hz displays.
    Vsync { base_ratio: f64 },
    /// Emulate at core fps, blocking on the audio queue; display shows the latest
    /// frame. Used for 120/144 Hz, odd panels (59/61 Hz) or unknown refresh.
    AudioSync { base_ratio: f64 },
}

impl SyncMode {
    /// Frequency ratio at the DRC target fill for this mode.
    pub fn base_ratio(self) -> f64 {
        match self {
            Self::Vsync { base_ratio } | Self::AudioSync { base_ratio } => base_ratio,
        }
    }
}

/// Picks the sync mode. `display_hz = None` (unknown, e.g. some Wayland setups)
/// or `force_audio_sync` → audio-sync, the mode that is correct everywhere.
pub fn select_sync_mode(
    display_hz: Option<f64>,
    core_fps: f64,
    core_rate: f64,
    force_audio_sync: bool,
) -> SyncMode {
    let audio_sync = SyncMode::AudioSync {
        base_ratio: audio_sync_ratio(core_rate),
    };
    let Some(hz) = display_hz.filter(|hz| *hz > 0.0) else {
        return audio_sync;
    };
    if force_audio_sync {
        return audio_sync;
    }
    let base_ratio = vsync_base_ratio(hz, core_fps, core_rate);
    if (base_ratio - 1.0).abs() <= VSYNC_TOLERANCE {
        SyncMode::Vsync { base_ratio }
    } else {
        audio_sync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FPS: f64 = 60.098_811_862_348_4;
    const RATE: f64 = 32_040.5;

    fn is_vsync(hz: Option<f64>) -> bool {
        matches!(
            select_sync_mode(hz, FPS, RATE, false),
            SyncMode::Vsync { .. }
        )
    }

    #[test]
    fn sixty_hz_class_displays_use_vsync() {
        assert!(is_vsync(Some(60.0)));
        assert!(is_vsync(Some(59.94)));
    }

    #[test]
    fn other_refresh_rates_use_audio_sync() {
        for hz in [59.0, 61.0, 75.0, 120.0, 144.0, 165.0] {
            assert!(!is_vsync(Some(hz)), "{hz} Hz");
        }
        assert!(!is_vsync(None));
    }

    #[test]
    fn force_flag_and_audio_sync_ratio() {
        let mode = select_sync_mode(Some(60.0), FPS, RATE, true);
        assert_eq!(
            mode,
            SyncMode::AudioSync {
                base_ratio: audio_sync_ratio(RATE)
            }
        );
        let at_144 = select_sync_mode(Some(144.0), FPS, RATE, false);
        assert_eq!(at_144.base_ratio(), audio_sync_ratio(RATE));
    }
}
