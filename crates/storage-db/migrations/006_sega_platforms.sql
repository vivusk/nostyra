-- M9: Sega 8/16-bit cartridge platforms, all played by Genesis Plus GX. The system
-- registry (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('genesis', 'MD', 'md,gen,smd,bin', 'Sega - Mega Drive - Genesis.dat', 'genesis_plus_gx'),
    ('sms', 'SMS', 'sms', 'Sega - Master System - Mark III.dat', 'genesis_plus_gx'),
    ('gg', 'GG', 'gg', 'Sega - Game Gear.dat', 'genesis_plus_gx'),
    ('sg1000', 'SG', 'sg', 'Sega - SG-1000.dat', 'genesis_plus_gx');
