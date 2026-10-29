//! M9 exit criterion: one folder mixing SNES / GB / PS1 with every Sega format. Each
//! file lands in the right system by content: Mega Drive `.md` / `.gen` / `.smd` /
//! `.bin`, a PlayStation `.bin`, `.sms` / `.gg` / `.sg`; a `.bin` with neither the
//! `SEGA` header nor a disc signature is reported; a Markdown `README.md` is neither
//! listed nor reported; a Mega Drive `.bin` beside an unrelated same-stem `.cue`
//! stays a game.
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::collections::BTreeMap;
use std::path::Path;

use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
use game_library::discover_rom_files;
use system_catalog::{raw_to_smd, ALL_SYSTEMS};

fn mega_drive() -> Vec<u8> {
    let mut rom = vec![0x4Eu8; 0x20000];
    rom[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
    rom
}

fn ps1_disc(exe: &str) -> Vec<u8> {
    to_raw(
        &ps1_sectors(Some(US_LICENCE), &format!(r"BOOT = cdrom:\{exe};1")),
        2,
    )
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn sega_files_sort_into_their_systems_beside_other_systems() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-gl-sega-mixed-")
        .tempdir()
        .unwrap();
    let write = |rel: &str, bytes: &[u8]| {
        let path = temp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    };
    let md = mega_drive();
    let mut gb = vec![0u8; 32 * 1024];
    gb[0x104..0x108].copy_from_slice(&[0xCE, 0xED, 0x66, 0x66]);
    write("SNES/a.sfc", &vec![0u8; 64 * 1024]);
    write("Handheld/b.gb", &gb);
    write("PS1/game.bin", &ps1_disc("SLUS_000.01"));
    write("Sega/sonic.md", &md);
    write("Sega/streets.gen", &md);
    write("Sega/copier.smd", &raw_to_smd(&md));
    write("Mixed/genesis.bin", &md);
    write("Sega/ninja.sms", &vec![0u8; 0x8000]);
    write("Sega/pocket.gg", &vec![0u8; 0x8000]);
    write("Sega/classic.sg", &vec![0u8; 0x8000]);
    write("Mixed/junk.bin", &vec![0u8; 0x20000]);
    write(
        "Sega/README.md",
        "# Notes\n\nMy Mega Drive dumps.\n".repeat(30).as_bytes(),
    );
    write("Pair/Sonic.bin", &md);
    write("Pair/Sonic (Track 1).bin", &ps1_disc("SLUS_000.02"));
    write(
        "Pair/Sonic.cue",
        b"FILE \"Sonic (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    );

    let found = discover_rom_files(temp.path(), &ALL_SYSTEMS);
    let listed: BTreeMap<String, &str> = found
        .files
        .iter()
        .map(|file| (name(Path::new(&file.path)), file.system.id))
        .collect();
    let expected: BTreeMap<String, &str> = [
        ("a.sfc", "snes"),
        ("b.gb", "gb"),
        ("game.bin", "psx"),
        ("sonic.md", "genesis"),
        ("streets.gen", "genesis"),
        ("copier.smd", "genesis"),
        ("genesis.bin", "genesis"),
        ("ninja.sms", "sms"),
        ("pocket.gg", "gg"),
        ("classic.sg", "sg1000"),
        ("Sonic.bin", "genesis"),
        ("Sonic.cue", "psx"),
    ]
    .into_iter()
    .map(|(file, system)| (file.to_string(), system))
    .collect();
    assert_eq!(listed, expected);
    let reported: Vec<String> = found.failures.iter().map(|f| name(&f.path)).collect();
    assert_eq!(reported, ["junk.bin"], "{:?}", found.failures);
}
