-- M18: Sony PlayStation 2 (PCEE2, the PCSX2 libretro core). The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('ps2', 'PS2', 'iso,bin,cue,chd,cso,zso,m3u', 'Sony - PlayStation 2.dat', 'pcee2');
