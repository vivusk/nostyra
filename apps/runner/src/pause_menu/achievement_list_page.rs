//! Content of the pause menu's "Achievements" page, built from rc_client's list
//! (core achievements grouped by progress: active challenges, recently unlocked,
//! almost there, locked, unlocked, unsupported…) when the menu opens and again after
//! an unlock or set load while it is open.

use achievements::{AchievementBucket, GameInfo, GameSummary};

use crate::achievement_overlay::game_placard::{game_icon, mode_label};
use crate::achievement_overlay::BadgeKey;

/// `RC_CLIENT_ACHIEVEMENT_UNLOCKED_*` bits (1 softcore, 2 hardcore).
const UNLOCKED_SOFTCORE: u8 = 1;
const UNLOCKED_HARDCORE: u8 = 2;

/// Page header: game, mode and totals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListHeader {
    pub icon: Option<BadgeKey>,
    pub title: String,
    /// "Softcore · 3 of 10 unlocked · 25 of 100 points".
    pub summary: String,
}

/// One achievement row.
#[derive(Clone, Debug, PartialEq)]
pub struct ListRow {
    /// Unlocked badge, or RA's greyed `_lock` variant while locked.
    pub badge: Option<BadgeKey>,
    pub title: String,
    pub description: String,
    pub points: u32,
    /// Measured progress such as "5/10" (empty without a measure).
    pub progress: String,
    pub unlocked: bool,
}

/// A section label or an achievement row, in display order.
#[derive(Clone, Debug, PartialEq)]
pub enum ListEntry {
    Section(String),
    Row(ListRow),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AchievementListPage {
    pub header: ListHeader,
    pub entries: Vec<ListEntry>,
}

impl AchievementListPage {
    /// The page for `game` in the given mode (an unlock counts in the active mode).
    pub fn build(
        game: &GameInfo,
        summary: &GameSummary,
        hardcore: bool,
        buckets: Vec<AchievementBucket>,
    ) -> Self {
        let mode_bit = if hardcore {
            UNLOCKED_HARDCORE
        } else {
            UNLOCKED_SOFTCORE | UNLOCKED_HARDCORE
        };
        let header = ListHeader {
            icon: game_icon(game),
            title: game.title.clone(),
            summary: format!(
                "{} · {} of {} unlocked · {} of {} points",
                mode_label(hardcore),
                summary.num_unlocked_achievements,
                summary.num_core_achievements,
                summary.points_unlocked,
                summary.points_core
            ),
        };
        let mut entries = Vec::new();
        for bucket in buckets.into_iter().filter(|b| !b.achievements.is_empty()) {
            entries.push(ListEntry::Section(bucket.label));
            entries.extend(bucket.achievements.into_iter().map(|a| {
                let unlocked = a.unlocked & mode_bit != 0;
                ListEntry::Row(ListRow {
                    badge: (!a.badge_name.is_empty()).then_some(BadgeKey::Achievement {
                        name: a.badge_name,
                        locked: !unlocked,
                    }),
                    title: a.title,
                    description: a.description,
                    points: a.points,
                    progress: a.measured_progress,
                    unlocked,
                })
            }));
        }
        Self { header, entries }
    }

    /// Indexes of the selectable rows (sections are labels only).
    pub fn row_indexes(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| matches!(self.entries[i], ListEntry::Row(_)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use achievements::AchievementInfo;

    use super::*;

    fn bucket(label: &str, ids: &[(u32, u8)]) -> AchievementBucket {
        AchievementBucket {
            label: label.into(),
            achievements: ids
                .iter()
                .map(|&(id, unlocked)| AchievementInfo {
                    id,
                    title: format!("Ach {id}"),
                    badge_name: format!("{id:05}"),
                    points: 5,
                    unlocked,
                    ..AchievementInfo::default()
                })
                .collect(),
            ..AchievementBucket::default()
        }
    }

    #[test]
    fn sections_rows_and_badge_variants_follow_the_mode() {
        let game = GameInfo {
            title: "Test Game".into(),
            badge_name: "085573".into(),
            ..GameInfo::default()
        };
        let summary = GameSummary {
            num_core_achievements: 2,
            num_unlocked_achievements: 1,
            points_core: 10,
            points_unlocked: 5,
            ..GameSummary::default()
        };
        let buckets = vec![
            bucket("Locked", &[(102, 0)]),
            bucket("Empty", &[]),
            bucket("Unlocked", &[(101, UNLOCKED_SOFTCORE)]),
        ];
        let page = AchievementListPage::build(&game, &summary, false, buckets.clone());
        assert_eq!(
            page.header.summary,
            "Softcore · 1 of 2 unlocked · 5 of 10 points"
        );
        assert_eq!(page.entries.len(), 4, "empty bucket skipped");
        assert_eq!(page.row_indexes(), [1, 3]);
        let ListEntry::Row(locked) = &page.entries[1] else {
            panic!("row expected")
        };
        assert!(!locked.unlocked);
        assert_eq!(
            locked.badge,
            Some(BadgeKey::Achievement {
                name: "00102".into(),
                locked: true
            })
        );
        let hard = AchievementListPage::build(&game, &summary, true, buckets);
        let ListEntry::Row(soft_only) = &hard.entries[3] else {
            panic!("row expected")
        };
        assert!(
            !soft_only.unlocked,
            "a softcore unlock is locked in hardcore"
        );
    }
}
