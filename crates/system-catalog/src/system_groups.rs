//! Named groups of systems that other crates branch on, kept here so no crate
//! outside the registry hard-codes a system id (literal gate).

/// Systems whose games may be listed and played from a single-ROM `.zip` (opt-in
/// ZIP scanning, PR #1): cartridge systems whose core loads the inner ROM bytes (or,
/// for Handy, the extracted file). Atari members need a named extension: a zipped
/// `.bin` is never DAT-gated (it classifies as before M17).
pub const ZIP_ARCHIVE_SYSTEMS: [&str; 10] = [
    "nes",
    "snes",
    "gba",
    "genesis",
    "sms",
    "gg",
    "sg1000",
    "atari2600",
    "atari7800",
    "lynx",
];

/// Systems with large save states (tens to hundreds of MiB): the runner saves the
/// periodic resume point less often on them.
pub const LARGE_STATE_SYSTEMS: [&str; 5] = ["psp", "dreamcast", "ps2", "gc", "wii"];

/// Whether a single-ROM `.zip` of `system_id` can be listed and played.
pub fn plays_from_zip_archive(system_id: &str) -> bool {
    ZIP_ARCHIVE_SYSTEMS.contains(&system_id)
}

/// Whether `system_id` has large save states (see [`LARGE_STATE_SYSTEMS`]).
pub fn has_large_states(system_id: &str) -> bool {
    LARGE_STATE_SYSTEMS.contains(&system_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_specs::system;

    #[test]
    fn groups_name_registry_systems() {
        for id in ZIP_ARCHIVE_SYSTEMS.iter().chain(&LARGE_STATE_SYSTEMS) {
            assert!(system(id).is_some(), "{id}");
        }
        assert!(plays_from_zip_archive("snes") && !plays_from_zip_archive("gb"));
        assert!(plays_from_zip_archive("atari7800") && plays_from_zip_archive("lynx"));
        assert!(has_large_states("wii") && !has_large_states("snes"));
    }
}
