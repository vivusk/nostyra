-- M13: Nintendo DS, played by melonDS DS. The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('nds', 'NDS', 'nds,dsi,ids', 'Nintendo - Nintendo DS.dat', 'melonds_ds');
