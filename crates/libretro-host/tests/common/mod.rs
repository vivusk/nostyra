//! Shared helpers for the real-core integration tests (each test file is its own
//! process, so each may host one core session).

#![allow(dead_code)] // each test binary uses a different subset

use std::path::{Path, PathBuf};

use libretro_host::{
    CoreSession, HostError, NullAudioSink, NullInputSource, NullVideoSink, SessionConfig,
};

pub fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

pub fn core_path() -> PathBuf {
    let path = repo_path(&format!(
        "cores/snes9x_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    assert!(
        path.exists(),
        "missing {}; run scripts/fetch-snes9x-core.sh (or .ps1) first",
        path.display()
    );
    path
}

pub fn load(core: &Path) -> Result<CoreSession, HostError> {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    CoreSession::load(SessionConfig {
        core_path: core,
        rom_path: &repo_path("test-assets/roms/nostyra-test.sfc"),
        system_dir: &tmp,
        save_dir: &tmp,
        rom_data: None,
        staging_dir: None,
        core_options: &[],
        hw_render: None,
        keep_library_loaded: false,
    })
}

pub struct Sinks {
    pub video: NullVideoSink,
    pub audio: NullAudioSink,
    pub input: NullInputSource,
}

impl Sinks {
    pub fn new() -> Self {
        Self {
            video: NullVideoSink::default(),
            audio: NullAudioSink::default(),
            input: NullInputSource,
        }
    }

    /// Runs `n` frames, returning the checksum after each one.
    pub fn run(&mut self, session: &mut CoreSession, n: usize) -> Vec<u64> {
        (0..n)
            .map(|_| {
                session
                    .run_frame(&mut self.video, &mut self.audio, &mut self.input)
                    .unwrap();
                self.video.last_checksum
            })
            .collect()
    }
}
