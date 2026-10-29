//! Achievement badges and the game icon for the overlay and the pause-menu list.
//! Files live in the cache shared with the launcher
//! (`<data>/cache/achievement-badges/`, names from `achievements::badge_paths`); a
//! missing file is downloaded once per session as a low-priority job on the HTTP
//! worker (GET on the media server, never through rc_client), checked to decode,
//! written atomically, then announced back. Decoded 64×64 RGBA images stay in a small
//! LRU; anything missing draws as a grey placeholder until its file lands.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use achievements::{badge_path, badge_url, game_icon_path, game_icon_url, AchievementClient};
use video_renderer::OverlayCanvas;

use super::badge_download::{decode_badge, download_job};

/// Badge images kept decoded (each 16 KiB).
const MAX_DECODED: usize = 256;
/// Side of a decoded badge (RA badges are 64×64; anything else is resized).
pub const BADGE_SIZE: u32 = 64;
const PLACEHOLDER_RGBA: [u8; 4] = [86, 90, 102, 255];

/// One image: an achievement badge (locked = RA's greyed `_lock` variant) or the
/// game icon.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BadgeKey {
    Achievement { name: String, locked: bool },
    GameIcon(String),
}

impl BadgeKey {
    fn path(&self, data_root: &Path) -> Option<PathBuf> {
        match self {
            Self::Achievement { name, locked } => badge_path(data_root, name, *locked),
            Self::GameIcon(icon) => game_icon_path(data_root, icon),
        }
    }

    fn url(&self) -> Option<String> {
        match self {
            Self::Achievement { name, locked } => badge_url(name, *locked),
            Self::GameIcon(icon) => game_icon_url(icon),
        }
    }
}

/// Decoded RGBA8, [`BADGE_SIZE`] square.
pub struct BadgeImage {
    pub rgba: Vec<u8>,
}

pub struct BadgeCache {
    data_root: PathBuf,
    decoded: HashMap<BadgeKey, Rc<BadgeImage>>,
    /// Least recently used first.
    order: VecDeque<BadgeKey>,
    /// No file or undecodable: not retried until a download lands.
    missing: HashSet<BadgeKey>,
    /// Asked for this session (download queued or file already there).
    requested: HashSet<BadgeKey>,
    landed_tx: Sender<BadgeKey>,
    landed_rx: Receiver<BadgeKey>,
    /// Bumped when a download lands (views redraw).
    revision: u64,
}

impl BadgeCache {
    pub fn new(data_root: &Path) -> Self {
        let (landed_tx, landed_rx) = channel();
        Self {
            data_root: data_root.to_path_buf(),
            decoded: HashMap::new(),
            order: VecDeque::new(),
            missing: HashSet::new(),
            requested: HashSet::new(),
            landed_tx,
            landed_rx,
            revision: 0,
        }
    }

    /// Makes sure `key` is on disk: queues a download on `client`'s worker unless the
    /// file exists or it was asked already this session.
    pub fn request(&mut self, key: BadgeKey, client: &AchievementClient) {
        let (Some(path), Some(url)) = (key.path(&self.data_root), key.url()) else {
            return; // invalid server name: placeholder
        };
        if !self.requested.insert(key.clone()) || path.exists() {
            return;
        }
        client.run_background_job(download_job(url, path, key, self.landed_tx.clone()));
    }

    /// Picks up finished downloads; true when one landed (redraw).
    pub fn poll(&mut self) -> bool {
        let mut landed = false;
        while let Ok(key) = self.landed_rx.try_recv() {
            self.missing.remove(&key);
            landed = true;
        }
        if landed {
            self.revision += 1;
        }
        landed
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The decoded image of `key` (read from disk on first use), `None` while missing.
    pub fn get(&mut self, key: &BadgeKey) -> Option<Rc<BadgeImage>> {
        if let Some(image) = self.decoded.get(key).cloned() {
            self.order.retain(|k| k != key);
            self.order.push_back(key.clone());
            return Some(image);
        }
        if self.missing.contains(key) {
            return None;
        }
        let image = key
            .path(&self.data_root)
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| decode_badge(&bytes));
        let Some(image) = image.map(Rc::new) else {
            self.missing.insert(key.clone());
            return None;
        };
        if self.decoded.len() >= MAX_DECODED {
            if let Some(oldest) = self.order.pop_front() {
                self.decoded.remove(&oldest);
            }
        }
        self.decoded.insert(key.clone(), image.clone());
        self.order.push_back(key.clone());
        Some(image)
    }

    /// Draws `key` scaled into `rect` (x, y, w, h), or the grey placeholder.
    pub fn draw(&mut self, canvas: &mut OverlayCanvas, key: Option<&BadgeKey>, rect: [u32; 4]) {
        match key.and_then(|key| self.get(key)) {
            Some(image) => canvas.blit_rgba_scaled(&image.rgba, (BADGE_SIZE, BADGE_SIZE), rect),
            None => canvas.fill_rect(rect[0], rect[1], rect[2], rect[3], PLACEHOLDER_RGBA),
        }
    }
}

#[cfg(test)]
#[path = "badge_cache_tests.rs"]
pub(super) mod tests;
