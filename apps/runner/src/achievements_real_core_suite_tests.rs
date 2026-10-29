//! RetroAchievements end to end on the real core of every RA system (M15 phase 07),
//! without network: test ROM → our hash → canned fake set (answered only for that
//! hash) → memory map or SYSTEM_RAM fallback (lazy for Mupen64Plus-Next / PPSSPP; GBA
//! IWRAM and NDS main RAM only through the core's map) → `do_frame` → unlock on the
//! exact frame the RAM frame counter reaches the value → popup, `AchievementUnlocked`,
//! award sent, snapshot written, journal empty. Softcore from boot, then (core reset)
//! hardcore with the hidden flag forced: the leaderboard starts and submits (D3: RA
//! leaderboards run in hardcore only). N64 / Dreamcast use OpenGL where the GL gate
//! passes (N64 falls back to Angrylion, Dreamcast skips). A missing core skips.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::sync::PoisonError;

use achievements::PendingJournal;
use ipc_protocol::RunnerEvent;
use libretro_host::HwRenderProvider;

use crate::achievements_fake_set_session::RaCase;
use crate::achievements_real_core_checks::{run_mode, ModeRun};
use crate::achievements_real_core_loader::load_ra_core;
use crate::real_core_session_loader::gl_test_gate;
use crate::runner_real_core_suite::{scratch, CORE_LOCK};

/// Frames a mode may take: the counters reach the leaderboard submit value by ~40 from
/// boot; PPSSPP may expose memory only after its 8-bit counter passed the value, so the
/// budget covers one more wrap (256 frames). Each mode stops once it is done.
const MAX_FRAMES: u32 = 400;

/// Unlock frames pinned for the deterministic CPU-only cores (softcore from boot, hardcore
/// after the core reset): a change means the ROM, the core or the frame hook timing moved.
/// Cores whose boot length depends on the platform or on wall-clock (N64, PSP, Dreamcast,
/// PS1: GL backend, async boot, BIOS) are not pinned; the exact-frame guarantee is already
/// asserted against the independent counter probe in `check_mode`.
const UNLOCK_FRAMES: &[(&str, Option<u32>, Option<u32>)] = &[
    ("nes", Some(7), Some(8)),
    ("snes", Some(21), Some(21)),
    ("gb", Some(21), Some(21)),
    ("gbc", Some(21), Some(21)),
    ("gba", Some(21), Some(21)),
    ("genesis", Some(21), Some(21)),
    ("sms", Some(21), Some(20)),
    ("gg", Some(21), Some(20)),
    ("sg1000", Some(21), Some(20)),
    ("psx", None, None),
    ("n64", None, None),
    ("dreamcast", None, None),
    ("psp", None, None),
    ("nds", Some(20), Some(20)),
];

fn run_case(system: &str) {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let case = RaCase::load(system);
    let wants_gl = matches!(system, "n64" | "dreamcast");
    let provider = (wants_gl && gl_test_gate()).then(hw_render_gl::SdlGlProvider::new);
    let gl = provider.as_ref().map(|p| p as &dyn HwRenderProvider);
    let mut core = match load_ra_core(system, &case.rom(), gl) {
        Ok(core) => core,
        Err(reason) => return eprintln!("{system}: skipped ({reason})"),
    };
    let data = scratch("ra-suite-data");
    let soft = run_mode(&case, &mut core, data.path(), false, MAX_FRAMES);
    check_mode(&case, &soft, false, data.path());
    core.session.reset();
    let hard = run_mode(&case, &mut core, data.path(), true, MAX_FRAMES);
    check_mode(&case, &hard, true, data.path());
    let frames = (soft.unlocked_at.unwrap(), hard.unlocked_at.unwrap());
    println!(
        "{system}: loaded at frame {:?}, unlocked at {} (softcore) / {} (hardcore)",
        soft.loaded_at, frames.0, frames.1
    );
    let &(_, soft_frame, hard_frame) = UNLOCK_FRAMES.iter().find(|(id, ..)| *id == system).unwrap();
    if let Some(expected) = hard_frame {
        assert_eq!(frames.1, expected, "{system}: hardcore unlock frame moved");
    }
    if let Some(expected) = soft_frame {
        assert_eq!(frames.0, expected, "{system}: softcore unlock frame moved");
    }
}

fn check_mode(case: &RaCase, run: &ModeRun, hardcore: bool, data: &Path) {
    let system = &case.system;
    let loaded = RunnerEvent::AchievementsLoaded {
        ra_game_id: case.game_id,
        unlocked: 0,
        total: 1,
        hardcore,
    };
    assert_eq!(run.events.first(), Some(&loaded), "{system}: {run:#?}");
    assert_eq!(
        run.loaded_at, run.regions_ready_at,
        "{system}: set requested and loaded (inline) on the first frame with memory"
    );
    let unlocked = RunnerEvent::AchievementUnlocked {
        id: case.achievement_id,
        hardcore,
    };
    assert!(run.events.contains(&unlocked), "{system}: {run:#?}");
    assert!(run.counter_hit_at.is_some(), "{system}: counter never hit");
    assert_eq!(
        run.unlocked_at, run.counter_hit_at,
        "{system}: unlock on the frame the counter reached {}",
        case.unlock_value
    );
    let award = run.field_of("awardachievement", "a");
    assert_eq!(award, Some(case.achievement_id.to_string()), "{system}");
    assert!(
        run.notices
            .iter()
            .any(|n| n.starts_with("Achievement unlocked")),
        "{system}: {:?}",
        run.notices
    );
    let snapshot = achievements::load_game_snapshot(data, &case.content_id())
        .unwrap()
        .expect("snapshot written");
    assert_eq!(
        (snapshot.ra_game_id, snapshot.hardcore),
        (case.game_id, hardcore)
    );
    let row = snapshot
        .achievements
        .iter()
        .find(|a| a.id == case.achievement_id);
    assert!(row.is_some_and(|a| a.unlocked), "{system}: {snapshot:?}");
    assert!(
        PendingJournal::new(data).list().is_empty(),
        "{system}: delivered"
    );
    if !hardcore {
        assert!(
            run.posts_of("submitlbentry").is_empty(),
            "{system}: softcore"
        );
        return;
    }
    for text in ["Leaderboard attempt started", "Leaderboard submitted"] {
        let shown = run.notices.iter().any(|n| n.starts_with(text));
        assert!(shown, "{system}: {text}: {:?}", run.notices);
    }
    let board = run.field_of("submitlbentry", "i");
    assert_eq!(board, Some(case.leaderboard_id.to_string()), "{system}");
    let score = run.field_of("submitlbentry", "s");
    assert_eq!(score, Some(case.lb_submit_value.to_string()), "{system}");
}

macro_rules! achievement_suites {
    ($($test:ident => $system:literal,)*) => {
        $(
            #[test]
            fn $test() {
                run_case($system);
            }
        )*
    };
}

achievement_suites! {
    achievements_real_core_nes => "nes",
    achievements_real_core_snes => "snes",
    achievements_real_core_gb => "gb",
    achievements_real_core_gbc => "gbc",
    achievements_real_core_gba => "gba",
    achievements_real_core_genesis => "genesis",
    achievements_real_core_sms => "sms",
    achievements_real_core_gg => "gg",
    achievements_real_core_sg1000 => "sg1000",
    achievements_real_core_psx => "psx",
    achievements_real_core_n64 => "n64",
    achievements_real_core_dreamcast => "dreamcast",
    achievements_real_core_psp => "psp",
    achievements_real_core_nds => "nds",
}
