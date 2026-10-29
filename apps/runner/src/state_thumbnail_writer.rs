//! Save-state thumbnails (`slotN.png`): the clean core frame (no OSD, no shader)
//! encoded as a native-size PNG on one long-lived worker thread, so F1 never waits
//! for the encoder. Files are written atomically; a failed thumbnail only logs a
//! warning (the state itself is already saved). Dropping the writer joins the
//! worker, so the last queued PNG is on disk before the runner exits.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;

use config_layers::write_atomically;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use video_renderer::RgbaFrameBuffer;

/// A copy of one RGBA8 frame, owned by the worker while it encodes (also what the
/// pause menu's slot grid shows).
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8 rows.
    pub rgba: Vec<u8>,
}

impl Thumbnail {
    /// Copies the latest frame; `None` before the core has produced one.
    pub fn from_frame(frame: &RgbaFrameBuffer) -> Option<Self> {
        let (width, height) = frame.size();
        (width > 0 && height > 0).then(|| Self {
            width,
            height,
            rgba: frame.pixels().to_vec(),
        })
    }
}

enum Job {
    Write(PathBuf, Thumbnail),
    /// No frame for this save: drop the old PNG so it can't mismatch the new state.
    Remove(PathBuf),
}

pub struct StateThumbnailWriter {
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
}

impl StateThumbnailWriter {
    /// Starts the worker; if the thread cannot be spawned, thumbnails are disabled.
    pub fn start() -> Self {
        let (jobs, inbox) = channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("state-thumbnails".into())
            .spawn(move || inbox.iter().for_each(run_job));
        match spawned {
            Ok(worker) => Self {
                jobs: Some(jobs),
                worker: Some(worker),
            },
            Err(err) => {
                tracing::warn!("state thumbnails disabled: {err}");
                Self {
                    jobs: None,
                    worker: None,
                }
            }
        }
    }

    /// Queues `path` to hold `thumbnail`, or to be removed when there is none.
    /// Jobs run in order, so a later save always wins.
    pub fn queue(&self, path: PathBuf, thumbnail: Option<Thumbnail>) {
        let job = match thumbnail {
            Some(thumbnail) => Job::Write(path, thumbnail),
            None => Job::Remove(path),
        };
        if let Some(jobs) = &self.jobs {
            if jobs.send(job).is_err() {
                tracing::warn!("state thumbnail worker is gone; thumbnail skipped");
            }
        }
    }
}

impl Drop for StateThumbnailWriter {
    /// Closes the queue and waits for pending thumbnails.
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                tracing::warn!("state thumbnail worker panicked");
            }
        }
    }
}

fn run_job(job: Job) {
    let result = match job {
        Job::Write(path, thumbnail) => write_png(&path, &thumbnail),
        Job::Remove(path) => match std::fs::remove_file(&path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                Err(anyhow::anyhow!("cannot remove {}: {err}", path.display()))
            }
            _ => Ok(()),
        },
    };
    if let Err(err) = result {
        tracing::warn!("state thumbnail failed: {err:#}");
    }
}

fn write_png(path: &Path, thumbnail: &Thumbnail) -> anyhow::Result<()> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive)
        .write_image(
            &thumbnail.rgba,
            thumbnail.width,
            thumbnail.height,
            ExtendedColorType::Rgba8,
        )?;
    write_atomically(path, &png)?;
    tracing::debug!(path = %path.display(), bytes = png.len(), "state thumbnail written");
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal

    use super::*;

    fn scratch(name: &str) -> tempfile::TempDir {
        let prefix = format!("nostyra-thumb-{name}-");
        tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
    }

    fn snes_frame() -> Thumbnail {
        let (width, height) = (256, 224);
        let rgba = (0..width * height)
            .flat_map(|i| [(i % 256) as u8, (i / 256) as u8, 0x40, 0xff])
            .collect();
        Thumbnail {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn writes_decodable_png_atomically_and_flushes_on_drop() {
        let temp = scratch("write");
        let dir = temp.path();
        let path = dir.join("slot0.png");
        let writer = StateThumbnailWriter::start();
        writer.queue(path.clone(), Some(snes_frame()));
        drop(writer); // must flush the pending job
        let decoded = image::load_from_memory(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 224));
        assert_eq!(decoded.to_rgba8().into_raw(), snes_frame().rgba);
        let names: Vec<_> = std::fs::read_dir(dir).unwrap().collect();
        assert_eq!(names.len(), 1, "no .tmp file left behind");
    }

    #[test]
    fn missing_frame_removes_stale_thumbnail() {
        let temp = scratch("remove");
        let dir = temp.path();
        let path = dir.join("slot1.png");
        std::fs::write(&path, b"old").unwrap();
        let writer = StateThumbnailWriter::start();
        writer.queue(path.clone(), None);
        writer.queue(dir.join("slot2.png"), None); // nothing to remove: no error
        drop(writer);
        assert!(!path.exists());
    }

    #[test]
    fn empty_frame_buffer_has_no_thumbnail() {
        assert!(Thumbnail::from_frame(&RgbaFrameBuffer::default()).is_none());
    }
}
