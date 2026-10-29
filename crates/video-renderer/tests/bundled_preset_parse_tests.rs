//! Every preset the config accepts by name (`config_layers::BUNDLED_PRESETS`) exists
//! under `third-party/slang-shaders` at its `bundled_preset_rel_path` (`crt/` or
//! `handheld/`), parses, and references only files that exist. No GPU needed: the
//! render checks stay `#[ignore]` in the crate's unit tests.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use librashader_presets::{ShaderFeatures, ShaderPreset};

/// Folders holding bundled `.slangp` files (upstream slang-shaders layout).
const PRESET_FOLDERS: [&str; 2] = ["crt", "handheld"];

fn shaders_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/slang-shaders")
}

#[test]
fn every_bundled_preset_parses_with_existing_sources() {
    assert!(!config_layers::BUNDLED_PRESETS.is_empty());
    for name in config_layers::BUNDLED_PRESETS {
        let rel = config_layers::bundled_preset_rel_path(name).expect("listed preset has a path");
        let path = shaders_dir().join(rel);
        let preset = ShaderPreset::try_parse(&path, ShaderFeatures::NONE)
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        assert!(!preset.passes.is_empty(), "{name}: no passes");
        assert_eq!(preset.pass_count as usize, preset.passes.len(), "{name}");
        let files = preset.passes.iter().map(|pass| &pass.path);
        for file in files.chain(preset.textures.iter().map(|tex| &tex.path)) {
            assert!(file.is_file(), "{name}: missing {}", file.display());
        }
    }
}

#[test]
fn no_unlisted_preset_is_bundled() {
    let mut on_disk: Vec<String> = PRESET_FOLDERS
        .iter()
        .flat_map(|folder| std::fs::read_dir(shaders_dir().join(folder)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension() == Some("slangp".as_ref()))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let mut listed: Vec<&str> = config_layers::BUNDLED_PRESETS.to_vec();
    listed.sort_unstable();
    assert_eq!(on_disk, listed, "presets folder and BUNDLED_PRESETS differ");
}
