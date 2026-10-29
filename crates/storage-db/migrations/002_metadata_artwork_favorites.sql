-- M2: offline game metadata, cached artwork, favorites. Append-only: never edit once released.

ALTER TABLE games ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;

-- One row per game with metadata. Index-derived rows (user_edited = 0) are rewritten by
-- scans and the startup self-heal; user-edited rows are frozen until the user resets
-- (reset = delete the row, then re-apply the vendored metadat).
-- Title override lives here because scans rewrite games.title on every rescan.
CREATE TABLE game_metadata (
    game_id                INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    title_override         TEXT,
    title_override_search  TEXT,            -- title_override lower-cased in Rust; NULL when no override
    developer              TEXT,
    publisher              TEXT,
    genre                  TEXT,
    release_year           INTEGER,
    players                INTEGER,
    description            TEXT,
    source_version         TEXT NOT NULL,   -- metadat commit that wrote the index fields
    user_edited            INTEGER NOT NULL DEFAULT 0,
    updated_at             INTEGER NOT NULL
);

-- Artwork lookup results per game and kind. Transient failures (offline, timeouts) write
-- no row, so the next fetch job retries them; 'not-found'/'failed' retry after 7 days.
CREATE TABLE artwork (
    game_id      INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL,             -- 'cover' | 'hero'
    status       TEXT NOT NULL,             -- 'cached' | 'not-found' | 'failed'
    local_path   TEXT,                      -- relative to <data>/artwork; NULL unless cached
    source       TEXT NOT NULL,             -- 'libretro-thumbnails'
    source_name  TEXT,                      -- thumbnail file name used
    checked_at   INTEGER NOT NULL,
    PRIMARY KEY (game_id, kind)
);

CREATE INDEX game_metadata_genre ON game_metadata(genre);
CREATE INDEX games_favorite ON games(favorite);
CREATE INDEX games_added_at ON games(added_at);
