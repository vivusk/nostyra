// Typed wrappers over the Rust commands (src-tauri/src/commands). Tauri maps
// camelCase argument names to the Rust snake_case parameters.
import { invoke } from "@tauri-apps/api/core";
import type { StateSlotId } from "./tauri-commands-gameplay";

export type GameSort = "recently-played" | "title" | "play-time" | "recently-added";
/** `played` = played at least once (Home only, not a Library pill). */
export type GameFilter = "all" | "recently-played" | "unplayed" | "needs-review" | "favorites" | "missing" | "played";

export interface GameListQuery {
  search?: string;
  sort: GameSort;
  filter: GameFilter;
  /** Exact genre; omitted = any. */
  genre?: string;
  /** Platform id (system filter); omitted = every system. */
  platform?: string;
  limit?: number;
}

export interface Game {
  id: number;
  platformId: string;
  folderId: number | null;
  romPath: string;
  fileSize: number;
  crc32: string;
  sha1: string;
  title: string;
  region: string | null;
  /** Full No-Intro name; null = not identified ("needs review"). */
  datName: string | null;
  addedAt: number;
  /** File not found on the last scan. */
  missing: boolean;
  totalPlayS: number;
  lastPlayedAt: number | null;
  genre: string | null;
  releaseYear: number | null;
  developer: string | null;
  favorite: boolean;
  /** Absolute paths of cached art (null = none); pass through `assetUrl`. */
  coverPath: string | null;
  heroPath: string | null;
  /** Changes only when the cover file is rewritten (cache-bust param). */
  coverCheckedAt: number | null;
}

export interface LibrarySummary {
  total: number;
  matched: number;
  needsReview: number;
  /** ROM file not found on the last scan. */
  missing: number;
}

export interface LibraryFolder {
  id: number;
  path: string;
  addedAt: number;
}

/** One registry core (`get_setup_state`); none is required to finish setup. */
export interface CoreSetupStatus {
  id: string;
  displayName: string;
  installed: boolean;
  /** System ids it plays. */
  systems: string[];
}

export interface SetupState {
  hasFolders: boolean;
  cores: CoreSetupStatus[];
  datMissing: boolean;
  dataRoot: string;
  scanning: boolean;
  artworkFetching: boolean;
  coreDownloading: boolean;
  activeGameId: number | null;
}

export interface CoreStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
  downloading: boolean;
}

export interface ScanProgress {
  folderPath: string;
  filesTotal: number;
  filesDone: number;
  currentPath: string | null;
}

/** Shape of every command error (Rust `CommandError`). */
export interface CommandError {
  kind: string;
  message: string;
  /** `core_missing` only: the core to install. */
  coreId?: string;
}

/** Readable text for anything a command may reject with. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  return String(error);
}

export const commands = {
  getSetupState: () => invoke<SetupState>("get_setup_state"),
  listLibraryFolders: () => invoke<LibraryFolder[]>("list_library_folders"),
  addLibraryFolder: (path: string) => invoke<LibraryFolder>("add_library_folder", { path }),
  removeLibraryFolder: (id: number) => invoke<boolean>("remove_library_folder", { id }),
  listGames: (query: GameListQuery) => invoke<Game[]>("list_games", { query }),
  librarySummary: () => invoke<LibrarySummary>("library_summary"),
  /** Deletes a missing game's row, play history, saves, config and cached art. */
  removeMissingGame: (gameId: number) => invoke<void>("remove_missing_game", { gameId }),
  /** Same for every missing game; returns how many were removed. */
  removeAllMissingGames: () => invoke<number>("remove_all_missing_games"),
  startScan: () => invoke<void>("start_scan"),
  cancelScan: () => invoke<void>("cancel_scan"),
  getScanProgress: () => invoke<ScanProgress | null>("get_scan_progress"),
  getCoreStatus: (coreId: string) => invoke<CoreStatus>("get_core_status", { coreId }),
  downloadCore: (coreId: string) => invoke<void>("download_core", { coreId }),
  /** Starts the game; `loadState` (0–9, or "auto" = Continue from the resume point)
   *  loads that save state at start. */
  playGame: (gameId: number, loadState?: StateSlotId) =>
    invoke<void>("play_game", { gameId, loadState: loadState ?? null }),
  getActiveSession: () => invoke<number | null>("get_active_session"),
};
