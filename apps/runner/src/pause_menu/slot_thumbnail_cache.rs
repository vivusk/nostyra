//! Slot thumbnails for the menu grid: each `slotN.png` is decoded at most once per
//! menu open, on first display. A save from the menu replaces the slot's entry
//! straight from the in-memory frame (the PNG is still being written on the
//! worker), so the grid never shows a stale or empty cell.

use std::path::PathBuf;

use crate::state_thumbnail_writer::Thumbnail;

enum Entry {
    NotLoaded,
    /// No file, or it could not be decoded.
    Missing,
    Image(Thumbnail),
}

pub struct SlotThumbnailCache {
    /// `slotN.png` per slot.
    paths: Vec<PathBuf>,
    entries: Vec<Entry>,
}

impl SlotThumbnailCache {
    /// Nothing is read until a cell is shown.
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let entries = paths.iter().map(|_| Entry::NotLoaded).collect();
        Self { paths, entries }
    }

    /// The thumbnail of `slot`, decoding its PNG on first use.
    pub fn get(&mut self, slot: usize) -> Option<&Thumbnail> {
        let entry = self.entries.get_mut(slot)?;
        if matches!(entry, Entry::NotLoaded) {
            *entry = self.paths.get(slot).map_or(Entry::Missing, |path| {
                decode_png(path).map_or(Entry::Missing, Entry::Image)
            });
        }
        match entry {
            Entry::Image(thumbnail) => Some(thumbnail),
            _ => None,
        }
    }

    /// The menu just saved `slot` with this frame (`None`: no frame, no thumbnail).
    pub fn store(&mut self, slot: usize, thumbnail: Option<Thumbnail>) {
        if let Some(entry) = self.entries.get_mut(slot) {
            *entry = thumbnail.map_or(Entry::Missing, Entry::Image);
        }
    }
}

fn decode_png(path: &std::path::Path) -> Option<Thumbnail> {
    if !path.exists() {
        return None;
    }
    match image::open(path) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            Some(Thumbnail {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            })
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), "cannot read state thumbnail: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal

    use super::*;

    fn thumb(width: u32, height: u32, value: u8) -> Thumbnail {
        let rgba = vec![value; (width * height * 4) as usize];
        Thumbnail {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn decodes_once_and_store_replaces_without_disk_read() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-menu-thumbs-")
            .tempdir()
            .unwrap();
        let paths: Vec<PathBuf> = (0..3)
            .map(|n| temp.path().join(format!("slot{n}.png")))
            .collect();
        image::RgbaImage::from_raw(4, 2, thumb(4, 2, 7).rgba)
            .unwrap()
            .save(&paths[1])
            .unwrap();
        let mut cache = SlotThumbnailCache::new(paths.clone());
        assert!(cache.get(0).is_none(), "no file: empty cell");
        assert_eq!(cache.get(1).unwrap().width, 4);
        std::fs::remove_file(&paths[1]).unwrap();
        assert_eq!(
            cache.get(1).unwrap().rgba[0],
            7,
            "decoded once, then cached"
        );

        // Menu save into slot 2: the PNG is not written yet, the cache has the frame.
        cache.store(2, Some(thumb(2, 2, 9)));
        assert!(!paths[2].exists());
        assert_eq!(cache.get(2).unwrap().rgba[0], 9);
        cache.store(1, None);
        assert!(cache.get(1).is_none());
        assert!(cache.get(99).is_none(), "out of range is harmless");
    }
}
