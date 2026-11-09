// Update checker commands (`update_check_commands.rs`). Desktop only: the web table
// answers `get_update_status` with null and rejects the rest.
import { invoke } from "../backend";

/** Last GitHub release seen (cached in settings, survives offline launches). */
export interface CachedRelease {
  /** Semver without the leading `v` (`0.4.0`). */
  version: string;
  title: string;
  /** GitHub release page; the backend allowlists it again before opening. */
  url: string;
  publishedAt: string | null;
  /** Plain-text notes excerpt (≤ 400 chars); render as text, never HTML. */
  excerpt: string;
  hasPortableZip: boolean;
}

export interface UpdateStatus {
  currentVersion: string;
  /** "Check for updates at startup". */
  enabled: boolean;
  /** Unix seconds of the last successful check (200 or 304). */
  lastCheckedAt: number | null;
  latest: CachedRelease | null;
  /** Latest is newer and has the portable zip; the skip is applied by the UI. */
  updateAvailable: boolean;
  skippedVersion: string | null;
  /** Only set by `checkForUpdates` (Check now) when GitHub could not be reached. */
  checkFailed: boolean;
}

export const updateCommands = {
  /** Status; runs the automatic check first when it is on and due (release builds). */
  getUpdateStatus: () => invoke<UpdateStatus | null>("get_update_status"),
  /** "Check now": ignores toggle and throttle. */
  checkForUpdates: () => invoke<UpdateStatus>("check_for_updates"),
  setUpdateCheckEnabled: (enabled: boolean) => invoke<UpdateStatus>("set_update_check_enabled", { enabled }),
  /** Rejects with an `invalid` error when `version` is not semver. */
  skipUpdateVersion: (version: string) => invoke<UpdateStatus>("skip_update_version", { version }),
  openUpdateReleasePage: () => invoke<void>("open_update_release_page"),
};
