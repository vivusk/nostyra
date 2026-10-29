// Settings → About → Logs commands (`log_commands.rs`).
import { invoke } from "@tauri-apps/api/core";

export const logCommands = {
  /** Opens `<data>/logs/` in the file manager. */
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  /** Writes a bug-report zip into `<data>/logs/`, reveals it and returns its path. */
  createBugReport: () => invoke<string>("create_bug_report"),
};
