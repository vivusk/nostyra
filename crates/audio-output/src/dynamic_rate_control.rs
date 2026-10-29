//! Pure math for audio/video timing (see system-architecture §3.1).
//!
//! SDL streams take an integer source rate, so the stream is opened at
//! `round(core_rate)` and every fractional/timing correction goes through the
//! stream's frequency ratio (>1 = consume input faster).

/// Max DRC deviation (±0.5%): inaudible pitch shift, enough to absorb drift.
pub const DEFAULT_MAX_DEVIATION: f64 = 0.005;

/// Ratio when emulation is paced by display vsync at `display_hz` instead of
/// `core_fps`: the core then produces `core_rate × display_hz / core_fps`
/// samples per second, which the stream (opened at `round(core_rate)`) must play.
pub fn vsync_base_ratio(display_hz: f64, core_fps: f64, core_rate: f64) -> f64 {
    (display_hz / core_fps) * audio_sync_ratio(core_rate)
}

/// Ratio when emulation runs at exactly `core_fps` (audio is the timing master):
/// only the fractional part of the core rate needs correcting.
pub fn audio_sync_ratio(core_rate: f64) -> f64 {
    core_rate / core_rate.round()
}

/// Dynamic rate control: nudges `base` so the queue converges on `target`.
///
/// `fill == target` → exactly `base`; fuller queue → >base (drain faster).
/// The correction is clamped to `±max_deviation`.
pub fn drc_ratio(fill: f64, target: f64, base: f64, max_deviation: f64) -> f64 {
    if target <= 0.0 {
        return base;
    }
    let correction =
        (max_deviation * (fill - target) / target).clamp(-max_deviation, max_deviation);
    base * (1.0 + correction)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNES_FPS: f64 = 60.098_811_862_348_4;
    const SNES_RATE: f64 = 32_040.5;
    const D: f64 = DEFAULT_MAX_DEVIATION;

    #[test]
    fn drc_is_base_at_target_and_clamped() {
        assert_eq!(drc_ratio(100.0, 100.0, 1.01, D), 1.01);
        assert!((drc_ratio(1_000.0, 100.0, 1.0, D) - 1.005).abs() < 1e-12);
        assert!((drc_ratio(0.0, 100.0, 1.0, D) - 0.995).abs() < 1e-12);
    }

    #[test]
    fn drc_is_monotonic_in_fill() {
        let ratios: Vec<f64> = (0..=20)
            .map(|f| drc_ratio(f as f64 * 10.0, 100.0, 1.0, D))
            .collect();
        assert!(ratios.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn vsync_base_for_common_refresh_rates() {
        // 60 Hz display runs the SNES ~0.16% slow → play slightly slower.
        let r60 = vsync_base_ratio(60.0, SNES_FPS, SNES_RATE);
        assert!((r60 - 0.998_340).abs() < 1e-5, "{r60}");
        let r5994 = vsync_base_ratio(59.94, SNES_FPS, SNES_RATE);
        assert!((r5994 - 0.997_342).abs() < 1e-5, "{r5994}");
        assert!(vsync_base_ratio(59.0, SNES_FPS, SNES_RATE) < 0.99);
        assert!(vsync_base_ratio(61.0, SNES_FPS, SNES_RATE) > 1.01);
    }

    #[test]
    fn audio_sync_ratio_has_no_display_factor() {
        // Same at any refresh (e.g. 144 Hz): only 32040.5 / 32041 (Rust rounds half away from zero).
        let r = audio_sync_ratio(SNES_RATE);
        assert!((r - 0.999_984_4).abs() < 1e-7, "{r}");
        assert_eq!(audio_sync_ratio(32_040.0), 1.0);
    }
}
