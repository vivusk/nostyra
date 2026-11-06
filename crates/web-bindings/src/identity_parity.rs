//! Content-id parity between the desktop (native) and the web (wasm32): the same
//! test ROMs must give the same identity. A native test computes them with the OS
//! files and keeps `test-assets/identity-expectations.json` current; the wasm test
//! (`wasm-pack test --node crates/web-bindings`) recomputes them from the same bytes
//! (embedded, no files on wasm32) through the virtual content source.

/// Test ROMs checked: (file name under `test-assets/roms`, system, bytes). `.cue`
/// entries are followed by the files they reference.
#[cfg(test)]
pub(crate) const PARITY_ROMS: [(&str, &str, &[u8]); 12] = [
    (
        "nostyra-test.nes",
        "nes",
        include_bytes!("../../../test-assets/roms/nostyra-test.nes"),
    ),
    (
        "nostyra-test.sfc",
        "snes",
        include_bytes!("../../../test-assets/roms/nostyra-test.sfc"),
    ),
    (
        "nostyra-test.gb",
        "gb",
        include_bytes!("../../../test-assets/roms/nostyra-test.gb"),
    ),
    (
        "nostyra-test.gbc",
        "gbc",
        include_bytes!("../../../test-assets/roms/nostyra-test.gbc"),
    ),
    (
        "nostyra-test.gba",
        "gba",
        include_bytes!("../../../test-assets/roms/nostyra-test.gba"),
    ),
    (
        "nostyra-test.gen",
        "genesis",
        include_bytes!("../../../test-assets/roms/nostyra-test.gen"),
    ),
    (
        "nostyra-test.sms",
        "sms",
        include_bytes!("../../../test-assets/roms/nostyra-test.sms"),
    ),
    (
        "nostyra-test.gg",
        "gg",
        include_bytes!("../../../test-assets/roms/nostyra-test.gg"),
    ),
    (
        "nostyra-test.sg",
        "sg1000",
        include_bytes!("../../../test-assets/roms/nostyra-test.sg"),
    ),
    (
        "nostyra-test-psx.cue",
        "psx",
        include_bytes!("../../../test-assets/roms/nostyra-test-psx.cue"),
    ),
    (
        "nostyra-test-psx.bin",
        "",
        include_bytes!("../../../test-assets/roms/nostyra-test-psx.bin"),
    ),
    (
        "nostyra-test-psx.m3u",
        "psx",
        include_bytes!("../../../test-assets/roms/nostyra-test-psx.m3u"),
    ),
];

/// Where the expectations live, relative to the crate.
#[cfg(test)]
pub(crate) const EXPECTATIONS_JSON: &str =
    include_str!("../../../test-assets/identity-expectations.json");

/// Identities of every [`PARITY_ROMS`] entry with a system, as `name → JSON`, read
/// from the current content source under `dir`.
#[cfg(test)]
pub(crate) fn identities(
    dir: &std::path::Path,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let empty = rom_identity::DatIndex::empty();
    PARITY_ROMS
        .iter()
        .filter(|(_, system, _)| !system.is_empty())
        .map(|(name, system, _)| {
            let game = crate::identity_json::identify(
                &dir.join(name),
                system,
                &empty,
                &rom_identity::MetadataIndex::empty(),
            )
            .unwrap_or_else(|err| panic!("{name}: {err}"));
            (
                (*name).to_string(),
                serde_json::to_value(game).unwrap_or_default(),
            )
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_tests {
    use std::path::Path;

    use super::*;

    /// `NOSTYRA_UPDATE_IDENTITY_EXPECTATIONS=1 cargo test -p web-bindings` rewrites
    /// the file after an intended identity change.
    #[test]
    fn identity_expectations_are_current() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
        let now = serde_json::to_string_pretty(&identities(&dir)).unwrap() + "\n";
        if std::env::var_os("NOSTYRA_UPDATE_IDENTITY_EXPECTATIONS").is_some() {
            let file = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../test-assets/identity-expectations.json");
            std::fs::write(file, &now).unwrap();
            return;
        }
        assert_eq!(
            EXPECTATIONS_JSON.replace("\r\n", "\n"),
            now,
            "run with NOSTYRA_UPDATE_IDENTITY_EXPECTATIONS=1"
        );
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use std::path::Path;

    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;

    #[wasm_bindgen_test]
    fn wasm_identity_equals_the_desktop_one() {
        crate::js_files::clear();
        for (name, _, bytes) in PARITY_ROMS {
            crate::js_files::add_bytes(&format!("/web/1/{name}"), bytes.to_vec());
        }
        let got = crate::js_files::with_files(|| identities(Path::new("/web/1")));
        let want: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(EXPECTATIONS_JSON).unwrap();
        assert_eq!(got, want);
    }
}
