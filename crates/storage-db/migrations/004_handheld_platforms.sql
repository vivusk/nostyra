-- M7: Game Boy, Game Boy Color and Game Boy Advance platform rows, all played by
-- mGBA. The system registry (crates/system-catalog) is the source of truth; this
-- table mirrors it. Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('gb', 'GB', 'gb', 'Nintendo - Game Boy.dat', 'mgba'),
    ('gbc', 'GBC', 'gbc', 'Nintendo - Game Boy Color.dat', 'mgba'),
    ('gba', 'GBA', 'gba', 'Nintendo - Game Boy Advance.dat', 'mgba');
