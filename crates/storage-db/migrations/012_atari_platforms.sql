-- M17: Atari 2600 (Stella), Atari 7800 (ProSystem) and Atari Lynx (Handy). The
-- system registry (crates/system-catalog) is the source of truth; this table
-- mirrors it. Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('atari2600', '2600', 'a26,bin', 'Atari - 2600.dat', 'stella'),
    ('atari7800', '7800', 'a78,bin', 'Atari - 7800.dat', 'prosystem'),
    ('lynx', 'Lynx', 'lnx,lyx,o,bll', 'Atari - Lynx.dat', 'handy');
