//! Track modes a `.cue` sheet declares (`MODE1/2352`, `AUDIO`, …).

/// Track storage as declared in the cue sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackMode {
    /// Raw sectors, user data at byte 16.
    Mode1Raw,
    /// Raw sectors, user data at byte 24 (PlayStation data tracks).
    Mode2Raw,
    /// 2048-byte user data only (ISO layout).
    Mode1Cooked,
    Audio,
    /// Any other mode (`MODE2/2336`, `CDG`, …): not readable here.
    Other,
}

impl TrackMode {
    pub(crate) fn parse(word: &str) -> Self {
        match word.to_ascii_uppercase().as_str() {
            "MODE1/2352" => Self::Mode1Raw,
            "MODE2/2352" => Self::Mode2Raw,
            "MODE1/2048" => Self::Mode1Cooked,
            "AUDIO" => Self::Audio,
            _ => Self::Other,
        }
    }

    /// Whether the track holds data this crate can read.
    pub fn is_data(self) -> bool {
        matches!(self, Self::Mode1Raw | Self::Mode2Raw | Self::Mode1Cooked)
    }
}
