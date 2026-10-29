//! Where the bundled No-Intro DATs and metadat files live at runtime. `tauri.conf.json`
//! `bundle.resources` copies `third-party/libretro-database/*` (and `metadat/**/*`)
//! into `<resource_dir>/libretro-database/` (tauri-build does the same next to the exe
//! in dev builds). The data is never compiled in, so users can replace it. File names
//! come from the system registry (`SystemSpec.dat_file` / `metadat_file`).

use std::path::{Path, PathBuf};

/// Resource sub-folder holding the DATs, `metadat/`, LICENSE and SOURCE.txt.
const DAT_RESOURCE_DIR: &str = "libretro-database";

/// Folder `game_library::SystemCatalogs` reads the DATs and `metadat/` from.
pub fn libretro_database_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join(DAT_RESOURCE_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_sits_in_its_resource_folder() {
        assert_eq!(
            libretro_database_dir(Path::new("res")),
            Path::new("res").join("libretro-database")
        );
    }

    #[test]
    fn bundled_files_of_every_system_exist_in_the_repo() {
        // Guards the bundle.resources globs against a vendored file being renamed.
        let dir = libretro_database_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third-party"),
        );
        for system in system_catalog::ALL_SYSTEMS {
            let dat = dir.join(system.dat_file);
            assert!(dat.is_file(), "{}", dat.display());
            let genre = dir.join("metadat/genre").join(system.metadat_file);
            assert!(genre.is_file(), "{}", genre.display());
        }
    }
}
