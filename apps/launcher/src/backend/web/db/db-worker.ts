// Web library database: SQLite (wasm) in OPFS through the opfs-sahpool VFS (no
// cross-origin isolation needed; one connection, hence the single-tab lock), with
// the desktop migrations applied as is. Requests arrive from db-client.ts.
/// <reference lib="webworker" />
import sqlite3InitModule from "@sqlite.org/sqlite-wasm";
import { MIGRATIONS } from "./db-migrations";
import { WEB_DB_FILE, WEB_DB_VFS, type DbRequest, type DbResponse, type SqlRow } from "./db-protocol";

type Database = Awaited<ReturnType<typeof open>>;

async function open() {
  const sqlite3 = await sqlite3InitModule();
  const pool = await sqlite3.installOpfsSAHPoolVfs({ name: WEB_DB_VFS });
  const db = new pool.OpfsSAHPoolDb(WEB_DB_FILE);
  db.exec("PRAGMA foreign_keys = ON");
  migrate(db);
  return db;
}

/** Applies the migrations past `user_version`, each in its own transaction. */
function migrate(db: { exec(sql: string): unknown; selectValue(sql: string): unknown; transaction(f: () => void): unknown }) {
  const applied = Number(db.selectValue("PRAGMA user_version") ?? 0);
  for (let index = applied; index < MIGRATIONS.length; index++) {
    db.transaction(() => {
      db.exec(MIGRATIONS[index]);
      db.exec(`PRAGMA user_version = ${index + 1}`);
    });
  }
}

const database = open();

function rows(db: Database, sql: string, params?: unknown[]): SqlRow[] {
  return db.selectObjects(sql, params as never) as SqlRow[];
}

async function handle(request: DbRequest): Promise<DbResponse> {
  try {
    const db = await database;
    if (request.kind === "query") {
      return { id: request.id, ok: true, rows: rows(db, request.sql, request.params) };
    }
    const results: SqlRow[][] = [];
    db.transaction(() => {
      for (const statement of request.statements) results.push(rows(db, statement.sql, statement.params));
    });
    return { id: request.id, ok: true, results };
  } catch (error) {
    return { id: request.id, ok: false, error: error instanceof Error ? error.message : String(error) };
  }
}

self.onmessage = (event: MessageEvent<DbRequest>) => {
  void handle(event.data).then((response) => self.postMessage(response));
};
