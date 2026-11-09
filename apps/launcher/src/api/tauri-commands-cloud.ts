// Cloud saves commands (src-tauri/src/commands/cloud_*_commands.rs). Desktop only:
// the web table answers the status queries with "off" and rejects the actions
// (`backend/web/desktop-only`); the UI never calls them unless `capabilities.cloudSaves`.
import { invoke } from "../backend";

export type CloudProviderId = "off" | "google_drive" | "onedrive" | "dropbox" | "local_folder";

/** Settings → Cloud saves view (`CloudStatusView`). */
export interface CloudStatus {
  provider: CloudProviderId;
  /** The provider is built into this version (Google Drive / OneDrive / Dropbox come later). */
  providerAvailable: boolean;
  accountLabel: string | null;
  /** `provider != off && !needsSignIn` (a reachable Local folder counts as signed in). */
  signedIn: boolean;
  needsSignIn: boolean;
  /** Why sign-in is needed ("The cloud folder is not reachable.", …). */
  signInReason: string | null;
  localFolder: string;
  syncStates: boolean;
  syncOnStart: boolean;
  /** A cloud job (pass, push, pre-play pull) runs right now. */
  syncing: boolean;
  /** Unix seconds of the last sync that reached the cloud. */
  lastSyncAt: number | null;
  /** "ok", "offline", "conflicts: 2", "sign in again", an error message… */
  lastResult: string | null;
  pendingUnits: number;
  conflictGames: number[];
}

/** One game's sync answer (`SyncGameResult`, tagged by `kind`). */
export type SyncGameResult =
  | { kind: "synced" }
  | { kind: "offline" }
  | { kind: "conflict"; files: string[]; localTime: number | null; remoteTime: number | null; remoteDevice: string | null }
  | { kind: "skipped"; reason: string };

export type CloudConflict = Extract<SyncGameResult, { kind: "conflict" }>;

/** "Keep this PC" / "Use cloud". */
export type ConflictChoice = "keepLocal" | "useRemote";

/** Detail chip state (`GameCloudStatus`). */
export interface GameCloudStatus {
  state: "off" | "synced" | "pending" | "conflict" | "error";
  error: string | null;
}

/** `[cloud]` toggles writable from Settings (global layer only). */
export type CloudToggleKey = "cloud.sync_states" | "cloud.sync_on_start";

export const cloudCommands = {
  status: () => invoke<CloudStatus>("cloud_status"),
  /** Local folder provider over `path` (rejects with `invalid` + a readable reason). */
  setLocalFolder: (path: string) => invoke<CloudStatus>("cloud_set_local_folder", { path }),
  /** Browser sign-in; rejects with `cloud_unavailable` until the provider ships. */
  signIn: (provider: CloudProviderId) => invoke<CloudStatus>("cloud_sign_in", { provider }),
  cancelSignIn: () => invoke<void>("cloud_cancel_sign_in"),
  /** Saves stay in the cloud and on this PC; the provider goes back to off. */
  signOut: () => invoke<CloudStatus>("cloud_sign_out"),
  gameStatus: (gameId: number) => invoke<GameCloudStatus>("cloud_game_status", { gameId }),
  /** Pre-play pull (pull only); answers within the backend's 15 s budget. */
  syncGame: (gameId: number) => invoke<SyncGameResult>("cloud_sync_game", { gameId }),
  /** `beforePlay`: Keep this PC uploads nothing now (after the session), Use cloud pulls. */
  resolveConflict: (gameId: number, choice: ConflictChoice, beforePlay: boolean) =>
    invoke<SyncGameResult>("cloud_resolve_conflict", { gameId, choice, beforePlay }),
  /** Every game in conflict with one choice (background pass). */
  resolveAll: (choice: ConflictChoice) => invoke<void>("cloud_resolve_all", { choice }),
  /** Library pass in the background (progress: `cloud-sync-progress`). */
  syncNow: () => invoke<void>("cloud_sync_now"),
  setToggle: (key: CloudToggleKey, value: boolean) =>
    invoke<void>("set_gameplay_config_value", { scope: "global", key, value }),
};
