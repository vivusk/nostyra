//! Where the bundled slang presets live at runtime. `tauri.conf.json`
//! `bundle.resources` copies `third-party/slang-shaders/` into
//! `<resource_dir>/shaders/` (tauri-build does the same next to the exe in dev
//! builds). The runner is always told the folder (`--shaders-dir`) because on macOS
//! (`Contents/Resources`) and Linux packages resources are not beside its exe.

use std::ffi::OsString;
use std::path::Path;

/// Resource sub-folder holding `crt/*.slangp`, `handheld/*.slangp` and SOURCE.txt.
const SHADERS_RESOURCE_DIR: &str = "shaders";

/// Extra runner arguments pointing at the bundled presets; empty when the folder
/// is missing (the runner then uses its own lookup and reports "Shaders not found").
pub fn runner_shader_args(resource_dir: Option<&Path>) -> Vec<OsString> {
    match resource_dir.map(|dir| dir.join(SHADERS_RESOURCE_DIR)) {
        Some(dir) if dir.is_dir() => vec!["--shaders-dir".into(), dir.into_os_string()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_the_folder_only_when_it_exists() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third-party");
        assert!(
            runner_shader_args(Some(&repo)).is_empty(),
            "no `shaders` folder there"
        );
        assert!(runner_shader_args(None).is_empty());
        let temp = tempfile::Builder::new()
            .prefix("nostyra-res-")
            .tempdir()
            .unwrap();
        let resources = temp.path();
        std::fs::create_dir_all(resources.join(SHADERS_RESOURCE_DIR)).unwrap();
        let args = runner_shader_args(Some(resources));
        assert_eq!(args[0], "--shaders-dir");
        assert_eq!(Path::new(&args[1]), resources.join(SHADERS_RESOURCE_DIR));
    }

    #[test]
    fn bundled_presets_exist_in_the_repo() {
        // Guards the bundle.resources source folder against renames.
        let preset = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../third-party/slang-shaders/crt/crt-easymode.slangp");
        assert!(preset.is_file(), "{}", preset.display());
    }
}
