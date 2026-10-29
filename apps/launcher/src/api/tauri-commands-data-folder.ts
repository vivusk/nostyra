// Settings → Data folder commands (`data_folder_commands.rs`).
import { invoke } from "@tauri-apps/api/core";

/** A picked folder, checked as the new data folder. */
export interface DataFolderCheck {
  /** Files of the current data folder a move would copy. */
  files: number;
  /** Of those, files that already exist in the picked folder. */
  conflicts: number;
}

/** `data-folder-move-progress` payload. */
export interface DataFolderMoveProgress {
  done: number;
  total: number;
}

export const DATA_FOLDER_MOVE_PROGRESS = "data-folder-move-progress";

export const dataFolderCommands = {
  /** Rejects ("invalid") a missing, unwritable or overlapping folder. */
  checkDataFolder: (path: string) => invoke<DataFolderCheck>("check_data_folder", { path }),
  /** Switches (optionally moving the data) and restarts the app: resolves only on error. */
  changeDataFolder: (path: string, moveData: boolean, overwrite: boolean) =>
    invoke<void>("change_data_folder", { path, moveData, overwrite }),
};
