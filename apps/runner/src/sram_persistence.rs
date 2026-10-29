//! Battery-backed cartridge memory: SRAM (`.srm`) and, for cores that keep it apart,
//! the cartridge clock (`rtc.bin`). Loaded right after `retro_load_game`, written
//! every 60 s if it changed (crash safety) and on exit. Writes are atomic.
//! [`sram_crc32`] fingerprints the live SRAM for state sidecars.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use config_layers::write_atomically;
use libretro_host::CoreSession;

const FLUSH_INTERVAL: Duration = Duration::from_secs(60);

/// Which core memory a [`SramPersistence`] mirrors to its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryMemory {
    /// `RETRO_MEMORY_SAVE_RAM` → `save.srm`.
    SaveRam,
    /// `RETRO_MEMORY_RTC` → `rtc.bin` (e.g. mGBA's Game Boy MBC3 clock).
    Rtc,
}

impl BatteryMemory {
    fn region(self, session: &mut CoreSession) -> Option<&mut [u8]> {
        match self {
            Self::SaveRam => session.save_ram_mut(),
            Self::Rtc => session.rtc_mut(),
        }
    }
}

pub struct SramPersistence {
    memory: BatteryMemory,
    path: PathBuf,
    /// Contents last written (or loaded); avoids rewriting unchanged memory.
    last_saved: Vec<u8>,
    last_check: Instant,
}

impl SramPersistence {
    /// Loads `path` into the core's `memory` if both exist. The core's size is read
    /// again at every save, so a buffer that settles later (mGBA reports 128 KiB for a
    /// GBA game until the game first touches its save chip) is written at its final
    /// size. A shorter file loads into the start of the buffer as is; a longer one
    /// (e.g. from another core) loads its prefix and is kept as `<file>.bak` first,
    /// since the next save would truncate it.
    pub fn load(memory: BatteryMemory, path: PathBuf, session: &mut CoreSession) -> Self {
        let mut last_saved = Vec::new();
        if let Some(region) = memory.region(session) {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    if copy_saved_into(region, &bytes) {
                        tracing::warn!(
                            file = bytes.len(),
                            core = region.len(),
                            ?memory,
                            "save file larger than the core's"
                        );
                        let mut backup = path.clone().into_os_string();
                        backup.push(".bak");
                        if let Err(err) = std::fs::write(&backup, &bytes) {
                            tracing::warn!(path = ?backup, %err, "cannot back up save file");
                        }
                    }
                    tracing::info!(path = %path.display(), file = bytes.len(), core = region.len(), "battery save loaded");
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => tracing::warn!(path = %path.display(), %err, "cannot read save file"),
            }
            last_saved = region.to_vec();
        }
        Self {
            memory,
            path,
            last_saved,
            last_check: Instant::now(),
        }
    }

    /// Call every frame; flushes at most once per `FLUSH_INTERVAL`.
    pub fn flush_if_due(&mut self, session: &mut CoreSession) {
        if self.last_check.elapsed() >= FLUSH_INTERVAL {
            self.last_check = Instant::now();
            self.flush(session);
        }
    }

    /// Writes the memory at its current size if it changed since the last write
    /// (always on exit).
    pub fn flush(&mut self, session: &mut CoreSession) {
        let Some(region) = self.memory.region(session) else {
            return;
        };
        if region == self.last_saved.as_slice() {
            return;
        }
        match write_atomically(&self.path, region) {
            Ok(()) => {
                self.last_saved.clear();
                self.last_saved.extend_from_slice(region);
                tracing::info!(path = %self.path.display(), "battery save written");
            }
            Err(err) => tracing::error!("{err:#}"),
        }
    }
}

/// Copies a saved file into the core's buffer: all of it when it fits (a shorter file
/// fills the start, the rest keeps the core's contents), else its prefix. Returns
/// true when the file is longer than the buffer, i.e. the next save would truncate it
/// and the caller must back it up first.
fn copy_saved_into(region: &mut [u8], saved: &[u8]) -> bool {
    let n = saved.len().min(region.len());
    region[..n].copy_from_slice(&saved[..n]);
    saved.len() > region.len()
}

/// CRC32 of the core's SRAM as it is now (what the next flush writes), `None` when
/// the game has no SRAM. Recorded in state sidecars so the launcher can spot a
/// `.srm` newer than a resume point.
pub fn sram_crc32(session: &mut CoreSession) -> Option<u32> {
    session.save_ram_mut().map(|sram| crc32fast::hash(sram))
}

#[cfg(test)]
mod tests {
    use super::copy_saved_into;

    #[test]
    fn saved_file_shorter_equal_or_longer_than_the_buffer() {
        // mGBA GBA: 32 KiB .srm into the 128 KiB buffer reported before detection.
        let mut region = vec![0xFF; 8];
        assert!(!copy_saved_into(&mut region, &[1, 2, 3]));
        assert_eq!(region, [1, 2, 3, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let mut region = vec![0; 3];
        assert!(!copy_saved_into(&mut region, &[4, 5, 6]));
        assert_eq!(region, [4, 5, 6]);
        // Longer (e.g. another core's file): prefix loaded, backup needed.
        let mut region = vec![0; 2];
        assert!(copy_saved_into(&mut region, &[7, 8, 9]));
        assert_eq!(region, [7, 8]);
    }
}
