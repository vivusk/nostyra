//! DAT-gated `.bin` lookup and the problem hint.

use system_catalog::{dat_identified_candidates, SystemSpec, ALL_SYSTEMS, ATARI2600, ATARI7800};

use super::*;

fn image(len: u32, seed: u32) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(seed) % 251) as u8)
        .collect()
}

fn dat(spec: &SystemSpec, rom: &[u8]) -> DatIndex {
    let hash = hash_rom_bytes(rom, spec);
    DatIndex::from_text(&format!(
        "game (\n\tname \"Fake (USA)\"\n\trom ( name \"Fake (USA).bin\" size {} crc {} sha1 {} )\n)\n",
        hash.size, hash.crc32, hash.sha1
    ))
    .unwrap()
}

#[test]
fn first_candidate_whose_dat_knows_the_file_wins() {
    let (vcs, prosystem) = (image(4096, 7), image(16384, 11));
    let dats = [
        (ATARI2600.id, dat(&ATARI2600, &vcs)),
        (ATARI7800.id, dat(&ATARI7800, &prosystem)),
    ];
    let dat_for = |spec: &SystemSpec| dats.iter().find(|(id, _)| *id == spec.id).map(|(_, d)| d);
    let cands = |len| dat_identified_candidates(&ALL_SYSTEMS, "bin", len);
    let hit = |bytes: &[u8]| dat_identified_system(bytes, &cands(bytes.len() as u64), dat_for);
    assert_eq!(hit(&vcs).map(|s| s.id), Some("atari2600"));
    assert_eq!(hit(&prosystem).map(|s| s.id), Some("atari7800"));
    assert_eq!(hit(&image(4096, 13)), None, "unknown dump");
    // No DAT loaded for a candidate: that candidate is skipped.
    assert_eq!(dat_identified_system(&vcs, &cands(4096), |_| None), None);
}

#[test]
fn a_headered_a78_image_hits_through_the_fallback() {
    let prosystem = image(16384, 11);
    let mut a78 = vec![0u8; 128];
    a78[1..10].copy_from_slice(b"ATARI7800");
    a78.extend_from_slice(&prosystem);
    let index = dat(&ATARI7800, &prosystem);
    let hit = dat_identified_system(&a78, &[&ATARI7800], |_| Some(&index));
    assert_eq!(hit.map(|s| s.id), Some("atari7800"));
}

#[test]
fn hint_names_the_candidates_and_their_homebrew_extensions() {
    assert_eq!(
        dat_pending_hint(
            "not a PlayStation or PlayStation 2 disc",
            &[&ATARI2600, &ATARI7800]
        ),
        "not a PlayStation or PlayStation 2 disc; also not a known Atari 2600 / Atari 7800 \
         dump (homebrew: use .a26 / .a78)"
    );
    assert_eq!(dat_pending_hint("x", &[]), "x");
}
