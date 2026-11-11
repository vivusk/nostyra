//! Plan acceptance "hot path allocation-free after warm-up": a host waiting in its
//! room polls the lobby (peers, messages, roster compare, launcher events) and
//! draws the HUD + status every redraw without one heap allocation once warm.
//! Counted per thread (`allocation-counter`, test builds only). ggrs' own
//! `advance_frame` request list is outside this check (crate internal).

use std::time::{Duration, Instant};

use netplay_session::lobby_state::LobbyPhase;
use netplay_session::room_api_types::SeatRole;
use netplay_session::session_fingerprint::{SessionFingerprint, PLATFORM};
use video_renderer::ViewportRect;

use super::NetplayOverlay;
use crate::netplay_runtime::fake_netplay_core::FakeNetplayCore;
use crate::netplay_runtime::local_mesh::LocalMesh;
use crate::netplay_runtime::{NetplayRuntime, RuntimeOptions};

fn options() -> RuntimeOptions {
    RuntimeOptions {
        role: SeatRole::Host,
        nickname: "host".into(),
        fingerprint: SessionFingerprint {
            core_id: "fake".into(),
            core_version: "1".into(),
            core_sha256: "00".into(),
            content_sha1: "11".into(),
            content_crc32: Some(7),
            patch_crc32: None,
            bios_md5s: Vec::new(),
            options_hash: 3,
            system: "snes".into(),
            fps_milli: 60_000,
            platform: PLATFORM.into(),
            analog: false,
            warmup_frames: 0,
            max_players: 2,
        },
        sram_len: 0,
        rtc_len: None,
        input_delay: Some(1),
        autostart: None,
        frames_limit: None,
        experimental: false,
        room_code: None,
    }
}

#[test]
fn idle_lobby_poll_and_hud_redraws_do_not_allocate() {
    let mesh = LocalMesh::new(1);
    let mut now = Instant::now();
    let mut runtime = NetplayRuntime::new(mesh.join(1), options(), now);
    let mut core = FakeNetplayCore::new(0);
    let mut overlay = NetplayOverlay::default();
    let game = ViewportRect {
        x: 0.0,
        y: 0.0,
        width: 640.0,
        height: 480.0,
    };
    let mut redraw = |runtime: &mut NetplayRuntime, now: Instant| {
        mesh.advance();
        runtime.poll_lobby(&mut core, now);
        let _ = runtime.take_events();
        let hud = (true, Some("Saved to slot 1"));
        assert!(overlay.hud(((640, 480), game), runtime, hud, now).is_some());
    };
    for _ in 0..10 {
        now += Duration::from_millis(16);
        redraw(&mut runtime, now); // warm-up: first roster, event queue, canvas
    }
    assert_eq!(*runtime.phase(), LobbyPhase::InLobby);
    let info = allocation_counter::measure(|| {
        for _ in 0..120 {
            now += Duration::from_millis(16);
            redraw(&mut runtime, now);
        }
    });
    assert_eq!(info.count_total, 0, "per-redraw allocations: {info:?}");
}
