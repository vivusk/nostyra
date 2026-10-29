-- M12: Sony PlayStation Portable, played by PPSSPP. The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('psp', 'PSP', 'iso,cso,chd,pbp', 'Sony - PlayStation Portable.dat', 'ppsspp');
