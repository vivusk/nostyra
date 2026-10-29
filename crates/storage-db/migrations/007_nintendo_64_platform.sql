-- M10: Nintendo 64, played by Mupen64Plus-Next. The system registry
-- (crates/system-catalog) is the source of truth; this table mirrors it.
-- Append-only: never edit once released.

INSERT INTO platforms (id, name, extensions, dat_file, core_id) VALUES
    ('n64', 'N64', 'z64,v64,n64', 'Nintendo - Nintendo 64.dat', 'mupen64plus-next');
