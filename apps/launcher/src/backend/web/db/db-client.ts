// Page side of the web library database: one module worker (db-worker.ts), calls
// matched to replies by id. Errors reject with the SQLite message.
import type { DbRequest, DbResponse, SqlParams, SqlRow, SqlStatement } from "./db-protocol";

/** `Omit` applied to each member of a union. */
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

type Pending = { resolve: (response: DbResponse & { ok: true }) => void; reject: (error: Error) => void };

let worker: Worker | null = null;
let nextId = 1;
const pending = new Map<number, Pending>();

function db(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL("./db-worker.ts", import.meta.url), { type: "module" });
  worker.onmessage = (event: MessageEvent<DbResponse>) => {
    const reply = event.data;
    const waiter = pending.get(reply.id);
    if (!waiter) return;
    pending.delete(reply.id);
    if (reply.ok) waiter.resolve(reply);
    else waiter.reject(new Error(reply.error));
  };
  worker.onerror = (event) => {
    for (const waiter of pending.values()) waiter.reject(new Error(`database worker failed: ${event.message}`));
    pending.clear();
  };
  return worker;
}

function send(request: DistributiveOmit<DbRequest, "id">): Promise<DbResponse & { ok: true }> {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    db().postMessage({ ...request, id } as DbRequest);
  });
}

/** Rows of one statement (objects keyed by column name). */
export async function dbQuery<T = SqlRow>(sql: string, params?: SqlParams): Promise<T[]> {
  const reply = await send({ kind: "query", sql, params });
  return ("rows" in reply ? reply.rows : []) as T[];
}

/** First row, or null. */
export async function dbQueryOne<T = SqlRow>(sql: string, params?: SqlParams): Promise<T | null> {
  return (await dbQuery<T>(sql, params))[0] ?? null;
}

/** Statements in one transaction (all or nothing); rows per statement. */
export async function dbBatch(statements: SqlStatement[]): Promise<SqlRow[][]> {
  const reply = await send({ kind: "batch", statements });
  return "results" in reply ? reply.results : [];
}
