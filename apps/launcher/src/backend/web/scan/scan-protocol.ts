// Messages between the launcher page (scan-command.ts: owns the database and the
// events) and the scan worker (scan-worker.ts: walks, classifies and hashes).
import type { ScanProgress } from "../../../api/tauri-commands";
import type { ScanProblem } from "../../../api/tauri-events";
import type { KnownFile } from "./scan-paths";

/** One library folder to scan. */
export interface ScanFolderInput {
  token: string;
  /** Shown in progress and problems (the browser never reveals full paths). */
  name: string;
  handle: FileSystemDirectoryHandle;
  /** Present rows of the folder by `rom_path` (rescan cache). */
  known: Record<string, KnownFile>;
}

export interface ScanRequest {
  kind: "start";
  folders: ScanFolderInput[];
  /** Shared setting `scan_zip_archives`. */
  zipScanning: boolean;
}

/** Index metadata of a matched game (`game_metadata` index columns). */
export interface ScannedMetadata {
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  releaseYear: number | null;
  players: number | null;
  sourceVersion: string;
}

/** One hashed game, ready to upsert (storage-db `ScannedGame`). */
export interface ScannedRow {
  romPath: string;
  platformId: string;
  fileSize: number;
  mtime: number;
  crc32: string;
  sha1: string;
  title: string;
  region: string | null;
  datName: string | null;
  serial: string | null;
  metadata: ScannedMetadata | null;
}

/** Pass A result of one folder: what is there now. */
export interface FolderDiscovered {
  kind: "discovered";
  token: string;
  /** `rom_path`s of the games found (visible entries). */
  present: string[];
  /** `rom_path` of every file walked (archives that still exist). */
  existing: string[];
  /** `rom_path`s the walk or the probe could not read: their games are kept. */
  unreadable: string[];
  skipped: number;
  /** Games of systems the browser cannot play (counted, not listed). */
  desktopOnly: number;
  failures: ScanProblem[];
}

export type ScanMessage =
  | { kind: "progress"; progress: ScanProgress }
  | FolderDiscovered
  | { kind: "folder-error"; token: string; error: string }
  | { kind: "games"; token: string; games: ScannedRow[]; failures: ScanProblem[]; progress: ScanProgress }
  | { kind: "done" }
  | { kind: "error"; error: string };

/** Files per database transaction (desktop `BATCH_SIZE`). */
export const SCAN_BATCH_SIZE = 50;
