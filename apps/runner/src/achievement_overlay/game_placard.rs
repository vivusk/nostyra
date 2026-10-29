//! Game-level popups: the start placard (icon, title, counts, Softcore / Hardcore
//! mode, or why achievements are inactive) and the mastery / completion popup.

use std::time::Duration;

use achievements::{GameInfo, GameSummary};

use super::badge_cache::BadgeKey;
use super::popup_queue::{Popup, PopupKind};

pub fn mode_label(hardcore: bool) -> &'static str {
    if hardcore {
        "Hardcore"
    } else {
        "Softcore"
    }
}

/// Game icon key (`None` without an icon name).
pub fn game_icon(game: &GameInfo) -> Option<BadgeKey> {
    (!game.badge_name.is_empty()).then(|| BadgeKey::GameIcon(game.badge_name.clone()))
}

/// Set loaded: "X of Y achievements · P points" in the given mode.
pub fn loaded(game: &GameInfo, summary: &GameSummary, hardcore: bool) -> Popup {
    let detail = if summary.num_core_achievements == 0 {
        "This game has no achievements yet".to_owned()
    } else {
        format!(
            "{} of {} achievements · {} points",
            summary.num_unlocked_achievements, summary.num_core_achievements, summary.points_core
        )
    };
    Popup {
        kind: PopupKind::Placard,
        icon: game_icon(game),
        header: format!("RetroAchievements · {} mode", mode_label(hardcore)),
        title: game.title.clone(),
        detail,
    }
}

/// The hash is not in RetroAchievements' database.
pub fn unknown(hash: &str) -> Popup {
    Popup {
        kind: PopupKind::Placard,
        icon: None,
        header: "RetroAchievements".into(),
        title: "Unknown game — achievements inactive".into(),
        detail: format!("Hash {hash}"),
    }
}

/// The core never exposed its memory (Q8).
pub fn memory_unavailable() -> Popup {
    Popup {
        kind: PopupKind::Placard,
        header: "RetroAchievements".into(),
        title: "Achievements unavailable with this core".into(),
        detail: "The core does not expose the console memory".into(),
        ..Popup::notice("")
    }
}

/// Every achievement unlocked: "Mastered" in hardcore, "Completed" in softcore.
pub fn mastery(game: &GameInfo, hardcore: bool, user: &str, played: Duration) -> Popup {
    let minutes = played.as_secs() / 60;
    let playtime = match minutes {
        0 => "under a minute".to_owned(),
        m if m < 60 => format!("{m} min"),
        m => format!("{} h {:02} min", m / 60, m % 60),
    };
    let verb = if hardcore { "Mastered" } else { "Completed" };
    Popup {
        kind: PopupKind::Mastery,
        icon: game_icon(game),
        header: format!("{verb} · {} mode", mode_label(hardcore)),
        title: game.title.clone(),
        detail: format!("{user} · played {playtime} this session"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> GameInfo {
        GameInfo {
            id: 1234,
            title: "Test Game".into(),
            badge_name: "085573".into(),
            ..GameInfo::default()
        }
    }

    #[test]
    fn placard_shows_counts_mode_and_icon() {
        let summary = GameSummary {
            num_core_achievements: 10,
            num_unlocked_achievements: 3,
            points_core: 100,
            ..GameSummary::default()
        };
        let popup = loaded(&game(), &summary, false);
        assert_eq!(popup.header, "RetroAchievements · Softcore mode");
        assert_eq!(popup.detail, "3 of 10 achievements · 100 points");
        assert_eq!(popup.icon, Some(BadgeKey::GameIcon("085573".into())));
        let empty = loaded(&game(), &GameSummary::default(), true);
        assert!(empty.header.contains("Hardcore") && empty.detail.contains("no achievements"));
        assert!(unknown("abc").detail.contains("abc"));
        assert_eq!(memory_unavailable().kind, PopupKind::Placard);
    }

    #[test]
    fn mastery_names_the_mode_user_and_playtime() {
        let hard = mastery(&game(), true, "Tester", Duration::from_secs(3_900));
        assert!(hard.header.starts_with("Mastered"));
        assert_eq!(hard.detail, "Tester · played 1 h 05 min this session");
        let soft = mastery(&game(), false, "Tester", Duration::from_secs(30));
        assert!(soft.header.starts_with("Completed") && soft.detail.contains("under a minute"));
    }
}
