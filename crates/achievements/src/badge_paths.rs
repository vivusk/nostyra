//! Where badge images live on the RA media server and in the local cache shared by
//! runner and launcher: `<data>/cache/achievement-badges/<badge>.png`,
//! `<badge>_lock.png`, and `game_<icon>.png` for the game icon. Names are validated so
//! a server-provided name can never escape the cache folder.

use std::path::{Path, PathBuf};

const MEDIA_HOST: &str = "https://media.retroachievements.org";

/// Cache folder for achievement badges and game icons.
pub fn badge_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cache").join("achievement-badges")
}

/// RA badge / icon names are short alphanumeric ids (`"00234"`); anything else is refused.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn badge_file_name(badge: &str, locked: bool) -> String {
    if locked {
        format!("{badge}_lock.png")
    } else {
        format!("{badge}.png")
    }
}

/// Cached badge file (locked = greyed variant); `None` for an invalid name.
pub fn badge_path(data_dir: &Path, badge: &str, locked: bool) -> Option<PathBuf> {
    valid_name(badge).then(|| badge_cache_dir(data_dir).join(badge_file_name(badge, locked)))
}

/// Badge download URL; `None` for an invalid name.
pub fn badge_url(badge: &str, locked: bool) -> Option<String> {
    valid_name(badge).then(|| format!("{MEDIA_HOST}/Badge/{}", badge_file_name(badge, locked)))
}

/// Cached game icon (`game_<icon>.png`); `None` for an invalid name.
pub fn game_icon_path(data_dir: &Path, icon: &str) -> Option<PathBuf> {
    valid_name(icon).then(|| badge_cache_dir(data_dir).join(format!("game_{icon}.png")))
}

/// Game icon download URL (`Images/<icon>.png`); `None` for an invalid name.
pub fn game_icon_url(icon: &str) -> Option<String> {
    valid_name(icon).then(|| format!("{MEDIA_HOST}/Images/{icon}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_urls() {
        let data = Path::new("data");
        let cache = data.join("cache").join("achievement-badges");
        assert_eq!(
            badge_path(data, "00234", false),
            Some(cache.join("00234.png"))
        );
        assert_eq!(
            badge_path(data, "00234", true),
            Some(cache.join("00234_lock.png"))
        );
        assert_eq!(
            game_icon_path(data, "085573"),
            Some(cache.join("game_085573.png"))
        );
        assert_eq!(
            badge_url("00234", true).as_deref(),
            Some("https://media.retroachievements.org/Badge/00234_lock.png")
        );
        assert_eq!(
            game_icon_url("085573").as_deref(),
            Some("https://media.retroachievements.org/Images/085573.png")
        );
    }

    #[test]
    fn unsafe_names_are_refused() {
        let data = Path::new("data");
        for bad in ["", "../x", "a/b", "a\\b", "x.png", &"9".repeat(40)] {
            assert_eq!(badge_path(data, bad, false), None, "{bad}");
            assert_eq!(badge_url(bad, false), None, "{bad}");
            assert_eq!(game_icon_path(data, bad), None, "{bad}");
        }
    }
}
