-- M1 schema. Migrations are append-only: never edit this file once released.
-- Times are unix seconds (INTEGER) unless the column name says otherwise.

CREATE TABLE platforms (
    id          TEXT PRIMARY KEY,           -- 'snes', 'nes'…
    name        TEXT NOT NULL,
    extensions  TEXT NOT NULL,              -- comma separated, lower case: 'sfc,smc'
    dat_file    TEXT,                       -- bundled DAT file name (resources)
    core_id     TEXT                        -- default core id; not an FK (cores row appears on download)
);

CREATE TABLE cores (
    id            TEXT PRIMARY KEY,
    platform_id   TEXT NOT NULL REFERENCES platforms(id),
    library_path  TEXT NOT NULL,
    version       TEXT,
    sha256        TEXT NOT NULL,
    installed_at  INTEGER NOT NULL
);

CREATE TABLE library_folders (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,
    added_at  INTEGER NOT NULL
);

CREATE TABLE games (
    id             INTEGER PRIMARY KEY,
    platform_id    TEXT NOT NULL REFERENCES platforms(id),
    -- NULL = folder removed: row hidden but kept so play history survives a re-add.
    folder_id      INTEGER REFERENCES library_folders(id) ON DELETE SET NULL,
    rom_path       TEXT NOT NULL UNIQUE,
    file_size      INTEGER NOT NULL,
    file_mtime_ns  INTEGER NOT NULL,
    crc32          TEXT NOT NULL,           -- upper-case hex of the headerless image
    sha1           TEXT NOT NULL,           -- upper-case hex of the headerless image
    title          TEXT NOT NULL,
    title_search   TEXT NOT NULL,           -- title lower-cased in Rust (Unicode-aware search)
    region         TEXT,
    dat_name       TEXT,                    -- NULL = not in DAT ("needs review")
    added_at       INTEGER NOT NULL,
    missing        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE play_sessions (
    id          INTEGER PRIMARY KEY,
    game_id     INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    started_at  INTEGER NOT NULL,
    duration_s  INTEGER NOT NULL,
    exit_code   INTEGER,
    source_key  TEXT UNIQUE                 -- pending-session file name: makes imports idempotent
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE INDEX games_title_search ON games(title_search);
CREATE INDEX games_sha1 ON games(sha1);
CREATE INDEX games_platform ON games(platform_id);
CREATE INDEX games_folder ON games(folder_id);
CREATE INDEX play_sessions_game_started ON play_sessions(game_id, started_at);

CREATE VIEW game_stats AS
SELECT game_id,
       SUM(duration_s)  AS total_play_s,
       MAX(started_at)  AS last_played_at,
       COUNT(*)         AS session_count
FROM play_sessions
GROUP BY game_id;

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('snes', 'Super Nintendo', 'sfc,smc', 'Nintendo - Super Nintendo Entertainment System.dat', 'snes9x');
