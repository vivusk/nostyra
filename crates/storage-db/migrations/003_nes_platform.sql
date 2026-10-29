-- M6: NES platform row (games.platform_id FK). The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id)
VALUES ('nes', 'NES', 'nes', 'Nintendo - Nintendo Entertainment System.dat', 'mesen');
