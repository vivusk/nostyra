// Play time of a web session: a `play_sessions` row in the web DB (keyed by the
// pending file name, like the desktop's import, so it is recorded once) and, when
// the Data folder is a real folder — possibly the desktop's — the same session as
// `sessions/pending/<ms>-web.json` (plan Q5): the desktop imports it and matches
// the game by content sha1 (its `rom_path` is a web path). The record's shape is
// serialized by the shared Rust type through wasm.
import { dbQuery } from "../db/db-client";
import { dataFolderStatus, currentDataDir } from "../fs/data-folder";
import { PENDING_SESSIONS_DIR } from "../fs/data-paths";
import { loadWasm } from "../wasm/wasm-module";

export interface FinishedSession {
  gameId: number;
  romPath: string;
  sha1: string;
  startedAtMs: number;
  endedAtMs: number;
}

export async function recordSession(session: FinishedSession): Promise<void> {
  const wasm = await loadWasm();
  const startedAt = Math.floor(session.startedAtMs / 1000);
  const seconds = Math.max(0, Math.round((session.endedAtMs - session.startedAtMs) / 1000));
  const fileName = wasm.web_pending_session_file_name(session.startedAtMs);
  await dbQuery(
    `INSERT OR IGNORE INTO play_sessions (game_id, started_at, duration_s, exit_code, source_key)
     VALUES (?, ?, ?, 0, ?)`,
    [session.gameId, startedAt, seconds, fileName],
  );
  const status = await dataFolderStatus();
  if (status.kind !== "folder" || status.needsAccess) return;
  const json = wasm.web_pending_session_json(session.romPath, startedAt, seconds, session.sha1);
  try {
    await (await currentDataDir()).writeFile(`${PENDING_SESSIONS_DIR}/${fileName}`, json);
  } catch (error) {
    console.warn("play session not shared with the desktop app", error);
  }
}
