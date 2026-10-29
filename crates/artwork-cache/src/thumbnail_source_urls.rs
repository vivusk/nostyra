//! Where libretro-thumbnails serves each system's art (per-system playlist folder
//! and GitHub repo from `SystemSpec`). Single place to change if the hosts move:
//! thumbnails.libretro.com is primary (no known throttling), the GitHub raw mirror
//! is the fallback (same files, synced source of truth), the GitHub tree API is the
//! file-name index.

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use system_catalog::SystemSpec;

/// Attribution / provenance label stored with each cached file.
pub const THUMBNAIL_SOURCE: &str = "libretro-thumbnails";

/// Recursive tree of the system's repo: every file name in one JSON response.
pub(crate) fn index_url(system: &SystemSpec) -> String {
    format!(
        "https://api.github.com/repos/libretro-thumbnails/{}/git/trees/master?recursive=1",
        system.thumbnails_repo
    )
}

/// Bytes kept literal in a URL path segment: unreserved characters plus the
/// parentheses, apostrophe, comma and `!` common in No-Intro names (both
/// hosts accept them raw); space, `#`, `%`, `+`, non-ASCII etc. are escaped.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'(')
    .remove(b')')
    .remove(b'\'')
    .remove(b',')
    .remove(b'!');

/// The three thumbnail folders Nostyra uses (logos are not needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailFolder {
    /// Box art: the cover.
    Boxarts,
    /// In-game screenshot: preferred hero image.
    Snaps,
    /// Title screen: hero fallback.
    Titles,
}

impl ThumbnailFolder {
    /// Folder name in the repo and on the server.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Boxarts => "Named_Boxarts",
            Self::Snaps => "Named_Snaps",
            Self::Titles => "Named_Titles",
        }
    }
}

/// Download URLs for `<folder>/<file_name>.png`, primary first then mirror.
/// `file_name` must already be sanitised (see `resolve_names`).
pub(crate) fn thumbnail_urls(
    system: &SystemSpec,
    folder: ThumbnailFolder,
    file_name: &str,
) -> [String; 2] {
    let file = utf8_percent_encode(file_name, PATH_SEGMENT);
    let dir = folder.dir_name();
    let playlist = utf8_percent_encode(system.thumbnails_folder, PATH_SEGMENT);
    let repo = system.thumbnails_repo;
    [
        format!("https://thumbnails.libretro.com/{playlist}/{dir}/{file}.png"),
        format!(
            "https://raw.githubusercontent.com/libretro-thumbnails/{repo}/master/{dir}/{file}.png"
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_primary_and_mirror_urls() {
        let [primary, mirror] = thumbnail_urls(
            &system_catalog::SNES,
            ThumbnailFolder::Boxarts,
            "Chrono Trigger (USA)",
        );
        assert_eq!(
            primary,
            "https://thumbnails.libretro.com/Nintendo%20-%20Super%20Nintendo%20Entertainment%20System/Named_Boxarts/Chrono%20Trigger%20(USA).png"
        );
        assert_eq!(
            mirror,
            "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Super_Nintendo_Entertainment_System/master/Named_Boxarts/Chrono%20Trigger%20(USA).png"
        );
    }

    #[test]
    fn escapes_reserved_and_unicode_characters() {
        let [primary, _] = thumbnail_urls(
            &system_catalog::SNES,
            ThumbnailFolder::Snaps,
            "Pop’n #1 + 50% (Japan)",
        );
        assert!(primary.ends_with("/Named_Snaps/Pop%E2%80%99n%20%231%20%2B%2050%25%20(Japan).png"));
    }

    #[test]
    fn nes_uses_its_own_repo_folder_and_index() {
        let nes = &system_catalog::NES;
        let [primary, mirror] =
            thumbnail_urls(nes, ThumbnailFolder::Boxarts, "Super Mario Bros. (World)");
        assert_eq!(
            primary,
            "https://thumbnails.libretro.com/Nintendo%20-%20Nintendo%20Entertainment%20System/Named_Boxarts/Super%20Mario%20Bros.%20(World).png"
        );
        assert_eq!(
            mirror,
            "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Nintendo_Entertainment_System/master/Named_Boxarts/Super%20Mario%20Bros.%20(World).png"
        );
        assert_eq!(
            index_url(nes),
            "https://api.github.com/repos/libretro-thumbnails/Nintendo_-_Nintendo_Entertainment_System/git/trees/master?recursive=1"
        );
    }
}
