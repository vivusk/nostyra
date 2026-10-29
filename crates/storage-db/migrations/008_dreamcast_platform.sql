-- M11: Sega Dreamcast, played by Flycast. The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('dreamcast', 'DC', 'gdi,cdi,chd,cue,m3u', 'Sega - Dreamcast.dat', 'flycast');
