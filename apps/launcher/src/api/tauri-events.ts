// Events pushed by the backend (src-tauri/src/app_events.rs).
import type { ScanProgress } from "./tauri-commands";

export const EVENTS = {
  scanProgress: "scan-progress",
  scanFinished: "scan-finished",
  coreDownloadProgress: "core-download-progress",
  coreDownloadFinished: "core-download-finished",
  sessionStarted: "session-started",
  sessionLaunchTimeout: "session-launch-timeout",
  sessionEnded: "session-ended",
  artworkProgress: "artwork-progress",
  artworkUpdated: "artwork-updated",
  artworkFinished: "artwork-finished",
  /** `{ gameId }`: the runner rewrote the game's achievement snapshot. */
  achievementsChanged: "achievements-changed",
  /** The stored RetroAchievements login was rejected and deleted. */
  raLoggedOut: "ra-logged-out",
} as const;

export type ScanProgressEvent = ScanProgress;

export interface ArtworkProgressEvent {
  done: number;
  total: number;
  currentTitle: string | null;
}

export interface ArtworkUpdatedEvent {
  gameId: number;
}

export interface ArtworkFinishedEvent {
  cached: number;
  notFound: number;
  failed: number;
  /** Offline / network trouble: nothing stored, retried next time. */
  transient: number;
  cancelled: boolean;
  error: string | null;
}

export interface ScanFinishedEvent {
  added: number;
  updated: number;
  missing: number;
  relinked: number;
  skipped: number;
  failedFiles: number;
  /** Why files were not added (first 100; `failedFiles` counts all). */
  failures: ScanProblem[];
  cancelled: boolean;
  error: string | null;
}

/** A file a scan could not add, with a readable reason. */
export interface ScanProblem {
  path: string;
  reason: string;
}

export interface CoreDownloadProgressEvent {
  coreId: string;
  bytesDone: number;
  bytesTotal: number | null;
}

export interface CoreDownloadFinishedEvent {
  coreId: string;
  error: string | null;
}

export interface SessionEvent {
  gameId: number;
}

export type SessionErrorKind =
  | "usage"
  | "core"
  | "rom"
  | "runtime"
  | "graphics"
  | "crashed";

export interface SessionEndedEvent {
  gameId: number;
  error: { kind: SessionErrorKind; message: string; detail: string } | null;
}
