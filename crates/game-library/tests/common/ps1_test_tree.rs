//! PS1 files for the mixed-tree scan test (synthetic discs, see disc-image test_images).

use super::TempLibrary;

pub const BROKEN_REASON: &str =
    "track file 'Brig_D13.bin' not found — fix the FILE line or rename the file";

/// M8: a cue game with an audio track, a playlist in the root with its discs in a
/// subfolder, a bare `.bin` and an `.iso`, a PS2-like disc, an audio-only `.bin`, and a
/// broken cue whose renamed track hides but whose neighbours stay listed.
pub fn write_ps1_tree(lib: &TempLibrary) {
    use disc_image::test_images::{ps1_sectors, to_iso, to_raw, US_LICENCE};
    let disc = |exe: &str| ps1_sectors(Some(US_LICENCE), &format!(r"BOOT = cdrom:\{exe};1"));
    let cue = |files: &[(&str, &str)]| -> String {
        files
            .iter()
            .enumerate()
            .map(|(i, (f, m))| {
                format!(
                    "FILE \"{f}\" BINARY
 TRACK {:02} {m}
  INDEX 01 00:00:00
",
                    i + 1
                )
            })
            .collect()
    };
    lib.write("PS1/Game (Track 1).bin", &to_raw(&disc("SLUS_000.01"), 2));
    lib.write("PS1/Game (Track 2).bin", &[3u8; 2352 * 8]);
    let game = cue(&[
        ("Game (Track 1).bin", "MODE2/2352"),
        ("Game (Track 2).bin", "AUDIO"),
    ]);
    lib.write("PS1/Game.cue", game.as_bytes());
    for n in [1, 2] {
        lib.write(
            &format!("Multi/D{n}.bin"),
            &to_raw(&disc(&format!("SLUS_100.0{n}")), 2),
        );
        let sheet = cue(&[(&format!("D{n}.bin"), "MODE2/2352")]);
        lib.write(&format!("Multi/D{n}.cue"), sheet.as_bytes());
    }
    lib.write(
        "Multi.m3u",
        b"Multi/D1.cue
Multi/D2.cue
",
    );
    lib.write("PS1/Bare.bin", &to_raw(&disc("SLUS_200.01"), 2));
    lib.write("PS1/Plain.iso", &to_iso(&disc("SLUS_300.01")));
    lib.write(
        "PS1/ps2.bin",
        &to_raw(&ps1_sectors(None, r"BOOT2 = cdrom0:\SLUS_400.01;1"), 2),
    );
    lib.write("PS1/audio.bin", &[9u8; 2352 * 40]);
    lib.write("PS1/Brig_TGGR.bin", &to_raw(&disc("SLUS_500.01"), 2));
    let broken = cue(&[("Brig_D13.bin", "MODE2/2352")]);
    lib.write("PS1/Brig_TGGR.cue", broken.as_bytes());
}
