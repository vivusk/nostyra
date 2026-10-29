//! Classifier tests: extension, size and probe per system, files on disk.

use std::cell::RefCell;
use std::path::PathBuf;

use super::*;
use crate::{DiscSignature, NoDiscProbe, PSX};

/// In-memory classification among every system (no file is opened).
fn head(ext: &str, len: u64, bytes: &[u8]) -> Result<&'static SystemSpec, ClassifyError> {
    classify_with_head(
        &ALL_SYSTEMS,
        ext,
        len,
        Some(bytes),
        Path::new("mem"),
        &NoDiscProbe,
    )
}

fn ines(prg_banks: u8) -> Vec<u8> {
    let mut rom = b"NES\x1A".to_vec();
    rom.extend_from_slice(&[prg_banks, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    rom.resize(16 + usize::from(prg_banks) * 16 * 1024 + 8 * 1024, 0xEA);
    rom
}

#[test]
fn classify_by_extension_size_and_magic() {
    let nes = ines(1);
    assert_eq!(
        head("NES", nes.len() as u64, &nes[..16]).map(|s| s.id),
        Ok("nes")
    );
    assert_eq!(
        head("nes", nes.len() as u64, &[0; 16]),
        Err(ClassifyError::ProbeFailed("missing iNES header"))
    );
    assert_eq!(head("nes", 100, &nes[..16]), Err(ClassifyError::OutOfSize));
    assert_eq!(head("sfc", 64 * 1024, &[]).map(|s| s.id), Ok("snes"));
    assert_eq!(head("txt", 64 * 1024, &[]), Err(ClassifyError::NoCandidate));
    // Restricted to SNES: a .nes file has no candidate.
    let snes_only = [&crate::SNES];
    assert_eq!(
        classify_with_head(
            &snes_only,
            "nes",
            nes.len() as u64,
            Some(&nes[..16]),
            Path::new("mem.nes"),
            &NoDiscProbe
        ),
        Err(ClassifyError::NoCandidate)
    );
}

/// 32 KiB GB/GBC image with (or without) the logo bytes mGBA checks.
fn gb_image(logo: bool) -> Vec<u8> {
    let mut rom = vec![0u8; 32 * 1024];
    if logo {
        rom[0x104..0x108].copy_from_slice(&[0xCE, 0xED, 0x66, 0x66]);
    }
    rom
}

/// GBA image starting with an ARM branch (byte 3 = 0xEA) or not.
fn gba_image(branch: bool) -> Vec<u8> {
    let mut rom = vec![0u8; 64 * 1024];
    if branch {
        rom[..4].copy_from_slice(&[0x2E, 0x00, 0x00, 0xEA]);
    }
    rom
}

#[test]
fn classify_handhelds() {
    let probe = |rom: &[u8]| rom[..PROBE_BYTES].to_vec();
    let gb = gb_image(true);
    assert_eq!(head("gb", 32 * 1024, &probe(&gb)).map(|s| s.id), Ok("gb"));
    assert_eq!(head("GBC", 32 * 1024, &probe(&gb)).map(|s| s.id), Ok("gbc"));
    assert_eq!(
        head("gb", 32 * 1024, &probe(&gb_image(false))),
        Err(ClassifyError::ProbeFailed("missing Game Boy header"))
    );
    let gba = gba_image(true);
    assert_eq!(
        head("GBA", 64 * 1024, &probe(&gba)).map(|s| s.id),
        Ok("gba")
    );
    assert_eq!(
        head("gba", 64 * 1024, &probe(&gba_image(false))),
        Err(ClassifyError::ProbeFailed("missing GBA header"))
    );
    // A short read (file smaller than the probe window) is judged on what was read.
    assert_eq!(
        head("gb", 32 * 1024, &probe(&gb)[..0x100]),
        Err(ClassifyError::ProbeFailed("missing Game Boy header"))
    );
    assert_eq!(head("gba", 1024, &gba[..8]).map(|s| s.id), Ok("gba"));
}

#[test]
fn classify_path_reads_content() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-classify-")
        .tempdir()
        .unwrap();
    let headered = temp.path().join("game.nes");
    std::fs::write(&headered, ines(2)).unwrap();
    let headerless = temp.path().join("bare.nes");
    std::fs::write(&headerless, &ines(2)[16..]).unwrap();
    // Folder names do not matter: an .sfc inside "NES" is SNES.
    let nes_dir = temp.path().join("NES");
    std::fs::create_dir(&nes_dir).unwrap();
    let sfc = nes_dir.join("x.sfc");
    std::fs::write(&sfc, vec![0u8; 64 * 1024]).unwrap();
    // The probe window reaches the GB logo bytes at 0x104; a 1 KiB GBA homebrew
    // (the size minimum) classifies too.
    let gb = temp.path().join("tetris.gb");
    std::fs::write(&gb, gb_image(true)).unwrap();
    let tiny_gba = temp.path().join("demo.gba");
    std::fs::write(&tiny_gba, &gba_image(true)[..1024]).unwrap();

    assert_eq!(
        classify_path(&headered, &NoDiscProbe).map(|s| s.id),
        Ok("nes")
    );
    assert_eq!(
        classify_path(&headerless, &NoDiscProbe),
        Err(ClassifyError::ProbeFailed("missing iNES header"))
    );
    assert_eq!(classify_path(&sfc, &NoDiscProbe).map(|s| s.id), Ok("snes"));
    assert_eq!(classify_path(&gb, &NoDiscProbe).map(|s| s.id), Ok("gb"));
    assert_eq!(
        classify_path(&tiny_gba, &NoDiscProbe).map(|s| s.id),
        Ok("gba")
    );
    assert!(matches!(
        classify_path(&temp.path().join("gone.nes"), &NoDiscProbe),
        Err(ClassifyError::Io(_))
    ));
    assert_eq!(
        classify_path(&temp.path().join("noext"), &NoDiscProbe),
        Err(ClassifyError::NoCandidate)
    );
}

/// Records the signatures it was asked about and answers with a fixed result.
struct FakeDisc {
    answer: Result<(), String>,
    asked: RefCell<Vec<DiscSignature>>,
}

impl FakeDisc {
    fn new(answer: Result<(), String>) -> Self {
        Self {
            answer,
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl DiscProbe for FakeDisc {
    fn check(&self, signature: DiscSignature, _path: &Path) -> Result<(), String> {
        self.asked.borrow_mut().push(signature);
        self.answer.clone()
    }
}

#[test]
fn disc_candidates_go_to_the_disc_probe() {
    let missing = PathBuf::from("no/such/dir/game.bin");
    let yes = FakeDisc::new(Ok(()));
    // The file does not exist: a disc candidate never reads the head itself.
    assert_eq!(
        classify("bin", 600_000_000, &missing, &yes).map(|s| s.id),
        Ok("psx")
    );
    assert_eq!(*yes.asked.borrow(), [DiscSignature::PlayStation]);
    let no = FakeDisc::new(Err("track file 'X.bin' not found".into()));
    assert_eq!(
        classify("cue", 900, &missing, &no),
        Err(ClassifyError::DiscRejected(
            "track file 'X.bin' not found".into()
        ))
    );
    assert_eq!(
        classify("iso", 8, &missing, &no),
        Err(ClassifyError::OutOfSize),
        "size checked before the disc probe"
    );
    // A `.cue` is a candidate of both disc systems: PlayStation is asked first.
    use DiscSignature::{Dreamcast, PlayStation};
    assert_eq!(*no.asked.borrow(), [PlayStation, Dreamcast]);
    let dc = FakeDisc::new(Ok(()));
    assert_eq!(
        classify("gdi", 120, &missing, &dc).map(|s| s.id),
        Ok("dreamcast")
    );
    assert_eq!(*dc.asked.borrow(), [Dreamcast], "a .gdi is Dreamcast only");
    assert_eq!(
        classify_path(&missing, &NoDiscProbe),
        Err(ClassifyError::Io(
            std::fs::metadata(&missing).unwrap_err().to_string()
        ))
    );
    assert_eq!(
        classify_among(&[&PSX], "m3u", 40, &missing, &NoDiscProbe),
        Err(ClassifyError::DiscRejected(
            "disc images are not supported here".into()
        ))
    );
}

#[test]
fn extension_only_candidates_never_open_the_file() {
    let missing = Path::new("no/such/dir/game.sfc");
    assert_eq!(
        classify("sfc", 64 * 1024, missing, &NoDiscProbe).map(|s| s.id),
        Ok("snes")
    );
    // A magic candidate reads the head lazily: missing file → Io.
    assert!(matches!(
        classify("nes", 40_976, Path::new("no/such/dir/x.nes"), &NoDiscProbe),
        Err(ClassifyError::Io(_))
    ));
}
