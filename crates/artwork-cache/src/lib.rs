//! Cover and hero art for identified games, from libretro-thumbnails.
//!
//! Tauri-free and DB-free: the launcher hands in a No-Intro name and a game
//! id, this crate resolves the thumbnail file name (GitHub tree index with
//! ETag cache, name sanitising, tag-strip fallback), downloads it over the
//! shared [`HttpClient`](http_download_util::HttpClient), validates and
//! re-encodes it with the pure-Rust `image` crate and writes it atomically to
//! `<data>/artwork/<game_id>-cover.jpg` / `<game_id>-hero.png`.
//!
//! Art is never bundled (libretro-thumbnails has no licence); it is fetched
//! per game at runtime into a user-clearable cache, with attribution shown by
//! the launcher. Blocking by design: callers run it on worker threads.

#![deny(unsafe_code)]

mod artwork_downloader;
mod artwork_error;
mod artwork_file_store;
mod artwork_image_processor;
mod artwork_request_and_outcome_types;
mod game_artwork_fetcher;
mod retail_name_filter;
mod thumbnail_index;
mod thumbnail_index_refresh;
mod thumbnail_name_sanitizer;
mod thumbnail_source_urls;

pub use artwork_error::ArtworkError;
pub use artwork_file_store::{
    artwork_cache_size, clear_artwork_cache, sweep_orphan_tmp, ArtworkCacheSize,
};
pub use artwork_image_processor::{encode_cover_jpeg, validate_hero_png, COVER_MAX_SIDE};
pub use artwork_request_and_outcome_types::{
    ArtworkKind, ArtworkOutcome, GameArtRequest, KindOutcome,
};
pub use game_artwork_fetcher::fetch_game_artwork;
pub use retail_name_filter::is_retail_name;
pub use thumbnail_index::ThumbnailIndex;
pub use thumbnail_index_refresh::{index_cache_is_fresh, INDEX_MAX_AGE};
pub use thumbnail_name_sanitizer::{fallback_names, resolve_names, sanitize_thumbnail_name};
pub use thumbnail_source_urls::{ThumbnailFolder, THUMBNAIL_SOURCE};
