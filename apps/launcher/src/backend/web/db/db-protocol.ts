// Messages between the launcher page and the SQLite worker (db-worker.ts).

export type SqlValue = string | number | null | Uint8Array;
export type SqlParams = SqlValue[];
export type SqlRow = Record<string, SqlValue>;

export interface SqlStatement {
  sql: string;
  params?: SqlParams;
}

export type DbRequest =
  | { id: number; kind: "query"; sql: string; params?: SqlParams }
  /** Runs in one transaction; each statement's rows come back (empty for writes). */
  | { id: number; kind: "batch"; statements: SqlStatement[] };

export type DbResponse =
  | { id: number; ok: true; rows: SqlRow[] }
  | { id: number; ok: true; results: SqlRow[][] }
  | { id: number; ok: false; error: string };

/** OPFS file of the web library (never the desktop `nostyra.db`). */
export const WEB_DB_FILE = "/nostyra-web.db";
/** opfs-sahpool VFS name (one per origin). */
export const WEB_DB_VFS = "nostyra-sahpool";
