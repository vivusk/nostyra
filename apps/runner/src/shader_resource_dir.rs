//! Where the bundled slang presets live and how a `video.shader` value becomes a
//! preset file. Lookup order (first existing folder wins): hidden `--shaders-dir`
//! (the launcher passes its resource folder: bundles keep resources away from the
//! exe on macOS/Linux) → `NOSTYRA_SHADERS` → `shaders/` next to the runner exe →
//! the repo's `third-party/slang-shaders` (debug builds only, for `cargo run`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use config_layers::{bundled_preset_rel_path, SHADER_NONE};
use video_renderer::ShaderChoice;

/// Environment variable naming the presets folder for a standalone runner.
pub const SHADERS_ENV: &str = "NOSTYRA_SHADERS";

/// No presets folder exists at any looked-up location.
#[derive(Debug, thiserror::Error)]
#[error("shader presets not found (looked in: {})", display_paths(.tried))]
pub struct ShadersNotFound {
    pub tried: Vec<PathBuf>,
}

/// Folders to try, in priority order, for this process.
pub fn candidate_dirs(cli_dir: Option<&Path>) -> Vec<PathBuf> {
    let env_dir = std::env::var_os(SHADERS_ENV);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let mut dirs = ordered_dirs(cli_dir, env_dir.as_deref(), exe_dir.as_deref());
    if cfg!(debug_assertions) {
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/slang-shaders"));
    }
    dirs
}

/// `--shaders-dir`, then the env folder, then `<exe dir>/shaders` (empty values skipped).
fn ordered_dirs(
    cli_dir: Option<&Path>,
    env_dir: Option<&OsStr>,
    exe_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let env_dir = env_dir.filter(|dir| !dir.is_empty()).map(Path::new);
    [cli_dir, env_dir]
        .into_iter()
        .flatten()
        .map(Path::to_path_buf)
        .chain(exe_dir.map(|dir| dir.join("shaders")))
        .collect()
}

/// Maps a validated `video.shader` value to a presenter choice: `none` → plain,
/// an absolute path → used as-is, a bundled name → `<first existing dir>/<its path in
/// bundled_preset_rel_path>` (`crt/…` or `handheld/…`).
pub fn resolve_shader(
    shader: &str,
    candidates: &[PathBuf],
) -> Result<ShaderChoice, ShadersNotFound> {
    if shader == SHADER_NONE {
        return Ok(ShaderChoice::None);
    }
    let as_path = Path::new(shader);
    if as_path.is_absolute() {
        return Ok(ShaderChoice::Preset(as_path.to_path_buf()));
    }
    candidates
        .iter()
        .find(|dir| dir.is_dir())
        .map(|dir| {
            // Validation only lets bundled names through; anything else is looked up
            // at the folder root (and then reported as a missing preset file).
            let rel = bundled_preset_rel_path(shader)
                .map_or_else(|| PathBuf::from(format!("{shader}.slangp")), PathBuf::from);
            ShaderChoice::Preset(dir.join(rel))
        })
        .ok_or_else(|| ShadersNotFound {
            tried: candidates.to_vec(),
        })
}

fn display_paths(paths: &[PathBuf]) -> String {
    let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    shown.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_dir_beats_env_beats_exe_dir() {
        let dirs = ordered_dirs(
            Some(Path::new("cli")),
            Some(OsStr::new("env")),
            Some(Path::new("exe")),
        );
        assert_eq!(
            dirs,
            [
                PathBuf::from("cli"),
                "env".into(),
                Path::new("exe").join("shaders")
            ]
        );
        let no_env = ordered_dirs(None, Some(OsStr::new("")), Some(Path::new("exe")));
        assert_eq!(no_env, [Path::new("exe").join("shaders")]);
    }

    #[test]
    fn names_resolve_in_the_first_existing_dir() {
        let existing = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let missing = existing.join("no-such-dir");
        let choice = resolve_shader("crt-geom", &[missing, existing.clone()]).unwrap();
        assert_eq!(
            choice,
            ShaderChoice::Preset(existing.join("crt").join("crt-geom.slangp"))
        );
        let lcd = resolve_shader("zfast-lcd", std::slice::from_ref(&existing)).unwrap();
        assert_eq!(
            lcd,
            ShaderChoice::Preset(existing.join("handheld").join("zfast-lcd.slangp"))
        );
    }

    #[test]
    fn none_and_absolute_paths_need_no_dir() {
        assert_eq!(resolve_shader("none", &[]).unwrap(), ShaderChoice::None);
        let abs = std::env::temp_dir().join("my.slangp");
        let choice = resolve_shader(abs.to_str().unwrap(), &[]).unwrap();
        assert_eq!(choice, ShaderChoice::Preset(abs));
    }

    #[test]
    fn nothing_found_lists_the_paths_tried() {
        let tried = vec![PathBuf::from("nowhere-a"), PathBuf::from("nowhere-b")];
        let err = resolve_shader("crt-lottes", &tried).unwrap_err();
        assert_eq!(err.tried, tried);
        assert!(err.to_string().contains("nowhere-a, nowhere-b"), "{err}");
    }

    #[test]
    #[cfg(debug_assertions)]
    fn debug_fallback_is_the_vendored_folder() {
        let dirs = candidate_dirs(None);
        let repo = dirs.last().unwrap();
        assert!(
            repo.join("crt/crt-easymode.slangp").is_file()
                && repo.join("handheld/zfast-lcd.slangp").is_file(),
            "{}",
            repo.display()
        );
    }
}
