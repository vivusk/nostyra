//! Achievement memory regions on every pinned core (M15 phase 02): the lazy policy
//! (`achievements::LazyMemoryRegions`) fed by the real session's memory map and
//! buffers must reach `Ready` within 60 frames, and known test-ROM bytes must sit at
//! their RA addresses (frame counters: `test-assets/achievements/generate-fake-sets.py`).
//! Ignored (needs the fetched cores); run explicitly:
//! `cargo test -p nostyra-runner ra_memory_regions -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::PoisonError;

use achievements::{LazyMemoryRegions, RegionStatus};
use libretro_host::HwRenderProvider;

use crate::achievements_real_core_loader::{load_ra_core, run_quiet_frame};
use crate::real_core_session_loader::gl_test_gate;
use crate::runner_real_core_suite::{repo, CORE_LOCK};

/// What a probe expects at an RA address after 60 frames.
#[derive(Clone, Copy)]
enum Probe {
    /// The byte differs between the first ready frame and frame 60 (frame counter).
    Changes(u32),
    /// The byte equals this value (save-RAM marker).
    Byte(u32, u8),
    /// The little-endian word equals this value (code word).
    Word(u32, u32),
}

const CASES: &[(&str, &str, &[Probe])] = &[
    (
        "nes",
        "nostyra-test.nes",
        &[Probe::Changes(0x0001), Probe::Byte(0x6001, 0xA5)],
    ),
    ("snes", "nostyra-test.sfc", &[Probe::Changes(0x0000)]),
    ("gb", "nostyra-test.gb", &[Probe::Changes(0xFF80)]),
    ("gbc", "nostyra-test.gbc", &[Probe::Changes(0xFF80)]),
    (
        "gba",
        "nostyra-test.gba",
        &[Probe::Changes(0x0), Probe::Byte(0x4_8001, 0xA5)],
    ),
    (
        "genesis",
        "nostyra-test.gen",
        &[Probe::Changes(0x0), Probe::Byte(0x1_0001, 0xA5)],
    ),
    (
        "sms",
        "nostyra-test.sms",
        &[Probe::Changes(0x0), Probe::Byte(0x2001, 0xA5)],
    ),
    (
        "gg",
        "nostyra-test.gg",
        &[Probe::Changes(0x0), Probe::Byte(0x2001, 0xA5)],
    ),
    ("sg1000", "nostyra-test.sg", &[Probe::Changes(0x0)]),
    ("psx", "nostyra-test-psx.cue", &[Probe::Changes(0xF_0000)]),
    ("n64", "nostyra-test.z64", &[Probe::Changes(0xF_0000)]),
    (
        "dreamcast",
        "nostyra-test-dc.gdi",
        &[Probe::Changes(0xF_0000)],
    ),
    // PPSSPP boots asynchronously: the counter may not run yet by frame 60 (the
    // achievement suite waits for it instead).
    ("psp", "nostyra-test-psp.iso", &[]),
    (
        "nds",
        "nostyra-test.nds",
        &[Probe::Word(0x0, 0xE59F_0128), Probe::Changes(0x3F_0008)],
    ),
];

#[test]
#[ignore = "needs the fetched cores; run explicitly"]
fn ra_memory_regions_reach_ready_on_every_core() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let provider = gl_test_gate().then(hw_render_gl::SdlGlProvider::new);
    for &(system_id, rom, probes) in CASES {
        let gl = provider.as_ref().map(|p| p as &dyn HwRenderProvider);
        check_system(system_id, rom, probes, gl);
    }
}

fn check_system(system_id: &str, rom: &str, probes: &[Probe], gl: Option<&dyn HwRenderProvider>) {
    let spec = system_catalog::system(system_id).unwrap();
    let rom = repo(&format!("test-assets/roms/{rom}"));
    let mut core = match load_ra_core(system_id, &rom, gl) {
        Ok(core) => core,
        Err(reason) => return println!("{system_id}: SKIPPED ({reason})"),
    };
    let session = &mut core.session;
    let console = u32::from(spec.ra_console_id.unwrap());
    let mut lazy = LazyMemoryRegions::new(console);
    let mut first_ready = None;
    let mut snapshot = Vec::new();
    for frame in 1..=60u32 {
        run_quiet_frame(session);
        let status = lazy.after_real_frame(session.memory_maps(), &|id| session.memory_info(id));
        if status == RegionStatus::Ready && first_ready.is_none() {
            first_ready = Some(frame);
            snapshot = probes.iter().map(|p| byte_at(&lazy, address(*p))).collect();
        }
    }
    let ready = first_ready.unwrap_or_else(|| panic!("{system_id}: regions never ready"));
    let regions = lazy.regions().unwrap();
    println!(
        "{system_id}: ready at frame {ready}, map generation {:?}, {:#x} bytes",
        regions.generation(),
        regions.total_size()
    );
    for (probe, before) in probes.iter().zip(snapshot) {
        match *probe {
            Probe::Changes(at) => assert_ne!(byte_at(&lazy, at), before, "{system_id} ${at:X}"),
            Probe::Byte(at, value) => {
                assert_eq!(byte_at(&lazy, at), Some(value), "{system_id} ${at:X}")
            }
            Probe::Word(at, value) => {
                let mut word = [0u8; 4];
                assert_eq!(lazy.read(at, &mut word), 4, "{system_id} ${at:X}");
                assert_eq!(u32::from_le_bytes(word), value, "{system_id} ${at:X}");
            }
        }
    }
}

fn address(probe: Probe) -> u32 {
    match probe {
        Probe::Changes(at) | Probe::Byte(at, _) | Probe::Word(at, _) => at,
    }
}

fn byte_at(lazy: &LazyMemoryRegions, address: u32) -> Option<u8> {
    let mut byte = [0u8; 1];
    (lazy.read(address, &mut byte) == 1).then_some(byte[0])
}
