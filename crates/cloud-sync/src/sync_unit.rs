//! What syncs as one piece: a game's save folder, keyed by system + content sha1
//! (the folder name depends on the ROM file stem, the content hash does not, so a
//! renamed ROM on another PC still finds its saves), or a shared folder used by many
//! games (the PS2 memory cards).

use std::path::{Path, PathBuf};

/// Key of the shared PS2 memory-card unit (`shared/ps2-memcards/` remotely).
pub const PS2_MEMCARDS_UNIT_KEY: &str = "ps2-memcards";

/// One sync unit and the local folder it mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncUnit {
    /// `<data>/saves/<system>/<rom key> [<ID8>]/` ↔ `games/<system>/<sha1>/`.
    Game {
        system: String,
        /// Full lower-case content sha1 (hex).
        content_sha1: String,
        local_dir: PathBuf,
    },
    /// A folder shared by several games ↔ `shared/<key>/`.
    Shared {
        key: &'static str,
        local_dir: PathBuf,
    },
}

impl SyncUnit {
    /// The unit of one game. `None` when the game has no usable content id
    /// (`ipc_protocol::content_id` is `None`) or the system id is not a plain name:
    /// the caller reports the game as skipped ("no content id").
    pub fn game(system: &str, content_sha1: &str, local_dir: PathBuf) -> Option<Self> {
        ipc_protocol::content_id(content_sha1)?;
        let hex = content_sha1.chars().all(|c| c.is_ascii_hexdigit());
        if !hex || !is_plain_name(system) {
            return None;
        }
        Some(Self::Game {
            system: system.to_string(),
            content_sha1: content_sha1.to_ascii_lowercase(),
            local_dir,
        })
    }

    /// The shared PS2 memory cards: `<data>/system/pcsx2/memcards/` (folder owned by
    /// `system-catalog`), synced with any PS2 game's pull / push.
    pub fn ps2_memcards(data_root: &Path) -> Self {
        Self::Shared {
            key: PS2_MEMCARDS_UNIT_KEY,
            local_dir: ipc_protocol::system_dir(data_root).join(system_catalog::PS2_MEMCARD_SUBDIR),
        }
    }

    /// Stable unit key: `<system>/<sha1>` or `shared/<key>`. Names the base manifest
    /// and the local backup folder, so both survive ROM renames.
    pub fn key(&self) -> String {
        match self {
            Self::Game {
                system,
                content_sha1,
                ..
            } => format!("{system}/{content_sha1}"),
            Self::Shared { key, .. } => format!("shared/{key}"),
        }
    }

    /// Folder of the unit under the remote root, with a trailing `/`:
    /// `games/<system>/<sha1>/` or `shared/<key>/`.
    pub fn remote_prefix(&self) -> String {
        match self {
            Self::Game { .. } => format!("games/{}/", self.key()),
            Self::Shared { .. } => format!("{}/", self.key()),
        }
    }

    /// The local folder the unit mirrors (may not exist yet).
    pub fn local_dir(&self) -> &Path {
        match self {
            Self::Game { local_dir, .. } | Self::Shared { local_dir, .. } => local_dir,
        }
    }
}

/// A single path segment of ASCII letters, digits, `-` or `_` (never `..` or `/`).
fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "6B47BB75D16514B6A476AA0C73A683A2A4C18765";

    #[test]
    fn game_unit_keys_use_the_content_hash() {
        let unit = SyncUnit::game("snes", SHA1, PathBuf::from("saves/x")).unwrap();
        let sha = SHA1.to_ascii_lowercase();
        assert_eq!(unit.key(), format!("snes/{sha}"));
        assert_eq!(unit.remote_prefix(), format!("games/snes/{sha}/"));
        assert_eq!(unit.local_dir(), Path::new("saves/x"));
    }

    #[test]
    fn no_content_id_or_odd_system_means_no_unit() {
        for (system, sha1) in [
            ("snes", ""),
            ("snes", "xyz"),
            ("snes", "6B47BB7"),
            ("../x", SHA1),
            ("", SHA1),
        ] {
            assert_eq!(
                SyncUnit::game(system, sha1, PathBuf::new()),
                None,
                "{system} {sha1}"
            );
        }
        assert_eq!(SyncUnit::game("snes", "6B47BB75zz", PathBuf::new()), None);
    }

    #[test]
    fn ps2_memcards_is_a_shared_unit_under_the_system_dir() {
        let root = Path::new("data");
        let unit = SyncUnit::ps2_memcards(root);
        assert_eq!(unit.key(), "shared/ps2-memcards");
        assert_eq!(unit.remote_prefix(), "shared/ps2-memcards/");
        assert!(unit.local_dir().starts_with(root.join("system")));
        assert!(unit.local_dir().ends_with("memcards"));
    }
}
