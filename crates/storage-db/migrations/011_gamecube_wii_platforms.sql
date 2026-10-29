-- M14: Nintendo GameCube and Wii, both played by Dolphin. The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('gc', 'GC', 'iso,gcm,ciso,gcz,rvz,wia,dol,elf,m3u', 'Nintendo - GameCube.dat', 'dolphin-emu'),
    ('wii', 'Wii', 'iso,ciso,gcz,rvz,wia,wbfs,dol,elf', 'Nintendo - Wii.dat', 'dolphin-emu');
