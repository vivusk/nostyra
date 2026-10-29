//! Disc systems in the registry, std-only: the media kind of a system, the disc
//! signature a probe asks for, and the trait that checks it. Reading disc images
//! (cue sheets, sectors, CHD) lives in the `disc-image` crate, which implements
//! [`DiscProbe`]; callers without it pass [`NoDiscProbe`].

use std::path::Path;

/// What a system's games are stored on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// One file = one game, hashed whole (or minus a copier header).
    Cartridge,
    /// CD / GD-ROM / UMD / GameCube / Wii disc images: a game may span several files
    /// (`.cue` + tracks, `.m3u` discs), identity = content hash + disc serial, no
    /// soft patching.
    Disc,
}

/// Which disc a [`ContentProbe::Disc`](crate::ContentProbe::Disc) accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscSignature {
    /// ISO9660 system id `PLAYSTATION` plus Sony's licence text in sector 4.
    PlayStation,
    /// IP.BIN hardware id `SEGA SEGAKATANA` in the first sector of the boot data track
    /// (GD-ROM high-density area at LBA 45000, or a MIL-CD's last session).
    Dreamcast,
    /// PSP UMD: ISO9660 system id `PSP GAME` or a `PSP_GAME` directory (`.iso`,
    /// `.cso`, DVD `.chd`), or an `EBOOT.PBP` that is not a PS1 classic.
    Psp,
    /// GameCube disc header magic `C2339F3D` @0x1C in a plain / CISO / GCZ / WIA /
    /// RVZ image (one 0x80-byte header read), or a PowerPC `.dol` / `.elf` without
    /// Wii-only code (HID4).
    GameCube,
    /// Wii disc header magic `5D1C9EA3` @0x18 in a plain / CISO / GCZ / WIA / RVZ /
    /// WBFS image, or a `.dol` / `.elf` with an `mtspr HID4` word in its code.
    Wii,
}

/// Checks a disc image on disk against a signature. `Err` carries the readable
/// reason shown in the scan problems list ("track file 'X' not found", "not a
/// PlayStation disc").
pub trait DiscProbe {
    fn check(&self, signature: DiscSignature, path: &Path) -> Result<(), String>;
}

/// For callers that never classify disc images.
pub struct NoDiscProbe;

impl DiscProbe for NoDiscProbe {
    fn check(&self, _signature: DiscSignature, _path: &Path) -> Result<(), String> {
        Err("disc images are not supported here".to_owned())
    }
}
