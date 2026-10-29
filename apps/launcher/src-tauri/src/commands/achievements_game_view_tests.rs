//! Achievements tab view: empty states per rule order, snapshot rows sorted unlocked
//! first, unofficial rows hidden, badge files resolved (lock variant or grey).

use achievements::{
    badge_path, game_icon_path, save_game_snapshot, GameSnapshot, SnapshotAchievement,
    SNAPSHOT_SCHEMA,
};

use super::*;

const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

fn target(system: &str, sha1: &str) -> GameDataTarget {
    GameDataTarget {
        system: system.into(),
        rom_path: "E:/ROMs/game.bin".into(),
        content_sha1: sha1.into(),
        missing: false,
    }
}

fn row(
    id: u32,
    badge: &str,
    unlocked: bool,
    unlock_time: i64,
    category: u8,
) -> SnapshotAchievement {
    SnapshotAchievement {
        id,
        title: format!("A{id}"),
        description: "d".into(),
        points: id,
        badge: badge.into(),
        unlocked,
        unlocked_hardcore: false,
        unlock_time,
        measured_progress: String::new(),
        category,
    }
}

fn write_snapshot(data: &Path) {
    let snapshot = GameSnapshot {
        schema: SNAPSHOT_SCHEMA,
        ra_game_id: 1,
        title: "Game".into(),
        icon_badge: "777".into(),
        hardcore: false,
        achievements: vec![
            row(1, "b1", false, 0, 1),
            row(2, "b2", true, 100, 1),
            row(3, "b3", false, 0, 1),
            row(4, "b4", true, 200, 1),
            row(5, "b5", true, 300, 2),
        ],
        updated_at: 500,
    };
    save_game_snapshot(data, SHA1, &snapshot).unwrap();
}

fn touch(path: Option<std::path::PathBuf>) {
    let path = path.unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"png").unwrap();
}

#[test]
fn empty_states_follow_the_rule_order() {
    let data = tempfile::tempdir().unwrap();
    write_snapshot(data.path());
    let gc = build_game_view(
        data.path(),
        &target(system_catalog::GC.id, SHA1),
        true,
        true,
    );
    assert_eq!(gc.status, AchievementsStatus::Unsupported);
    let logged_out = build_game_view(data.path(), &target("snes", SHA1), false, true);
    assert_eq!(logged_out.status, AchievementsStatus::LoggedOut);
    let other = "0000000000000000000000000000000000000000";
    let unplayed = build_game_view(data.path(), &target("snes", other), true, false);
    assert_eq!(
        (unplayed.status, unplayed.enabled),
        (AchievementsStatus::NotLoaded, false)
    );
    let bad = build_game_view(data.path(), &target("snes", "../../etc"), true, true);
    assert_eq!(bad.status, AchievementsStatus::NotLoaded);
}

#[test]
fn ready_view_sorts_counts_and_resolves_badges() {
    let data = tempfile::tempdir().unwrap();
    write_snapshot(data.path());
    touch(badge_path(data.path(), "b4", false));
    touch(badge_path(data.path(), "b1", true));
    touch(badge_path(data.path(), "b3", false));
    touch(game_icon_path(data.path(), "777"));

    let view = build_game_view(data.path(), &target("snes", SHA1), true, true);
    assert_eq!(view.status, AchievementsStatus::Ready);
    let ids: Vec<u32> = view.achievements.iter().map(|r| r.id).collect();
    assert_eq!(
        ids,
        [4, 2, 1, 3],
        "unlocked newest first, unofficial hidden"
    );
    let game = view.game.unwrap();
    assert_eq!((game.unlocked, game.total), (2, 4));
    assert_eq!((game.points_unlocked, game.points_total), (6, 10));
    assert!(game.icon_path.is_some());

    let by_id = |id: u32| view.achievements.iter().find(|r| r.id == id).unwrap();
    assert!(by_id(4).badge_path.as_deref().unwrap().ends_with("b4.png"));
    assert!(!by_id(4).badge_grayscale);
    assert_eq!(by_id(2).badge_path, None, "not downloaded");
    assert!(by_id(1)
        .badge_path
        .as_deref()
        .unwrap()
        .ends_with("b1_lock.png"));
    assert!(!by_id(1).badge_grayscale);
    assert!(by_id(3).badge_path.as_deref().unwrap().ends_with("b3.png"));
    assert!(
        by_id(3).badge_grayscale,
        "locked without a lock file → grey"
    );
}
