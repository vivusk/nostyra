-- M8: PlayStation platform row, played by PCSX ReARMed, and the disc serial column
-- (boot serial from SYSTEM.CNF, e.g. SLUS-01234; NULL for cartridge games). The
-- system registry (crates/system-catalog) is the source of truth; this table
-- mirrors it. Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id)
VALUES ('psx', 'PS1', 'cue,bin,iso,chd,pbp,m3u', 'Sony - PlayStation.dat', 'pcsx-rearmed');

ALTER TABLE games ADD COLUMN serial TEXT;
