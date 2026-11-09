//! PlayStation 2 video mode named by a disc's `SYSTEM.CNF` (`VMODE = NTSC` / `PAL`).
//! Owned here (std-only registry) so the disc reader that parses it and the BIOS
//! picker that consumes it (M18 phase 02) share one type without a crate cycle.

/// `VMODE` of a PS2 disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ps2VideoMode {
    Ntsc,
    Pal,
}

impl Ps2VideoMode {
    /// The mode a `VMODE` value names (case-insensitive, blanks ignored); `None` for
    /// anything else.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "NTSC" => Some(Self::Ntsc),
            "PAL" => Some(Self::Pal),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Ps2VideoMode;

    #[test]
    fn parses_both_modes_case_insensitively() {
        assert_eq!(Ps2VideoMode::parse(" ntsc "), Some(Ps2VideoMode::Ntsc));
        assert_eq!(Ps2VideoMode::parse("PAL"), Some(Ps2VideoMode::Pal));
        assert_eq!(Ps2VideoMode::parse("SECAM"), None);
        assert_eq!(Ps2VideoMode::parse(""), None);
    }
}
