// Commands only the web edition has: which Data folder the browser uses and
// reconnecting folders after a visit without permission (backend/web/data-folder),
// and the in-page player's Exit, slot picker and parked SRAM (backend/web/play).
import { invoke } from "../backend";
import type { StateSlotId } from "./tauri-commands-gameplay";

/** `web-slot-picker` event: open the picker for the running game; null closes it. */
export type WebSlotPickerRequest = { gameId: number; mode: "save" | "load" } | null;

export const WEB_EVENTS = {
  slotPicker: "web-slot-picker",
  /** `{ paths }`: a session could not write SRAM to the Data folder (kept in the browser). */
  pendingSaves: "web-pending-saves",
} as const;

export interface WebDataFolderStatus {
  kind: "browser" | "folder";
  name: string | null;
  sharedWithDesktop: boolean;
  needsAccess: boolean;
}

export interface WebDataFolderSwitch {
  copied: number;
  conflicts: string[];
}

export const webCommands = {
  dataFolderStatus: () => invoke<WebDataFolderStatus>("web_data_folder_status"),
  /** Opens the folder picker; `null` = cancelled (or refused by Chrome). */
  chooseDataFolder: () => invoke<WebDataFolderSwitch | null>("web_choose_data_folder"),
  useBrowserStorage: () => invoke<void>("web_use_browser_storage"),
  clearBrowserCopy: () => invoke<void>("web_clear_browser_copy"),
  foldersNeedingAccess: () => invoke<string[]>("web_folders_needing_access"),
  /** Resolves to how many folders are still not accessible. */
  reconnectFolders: () => invoke<number>("web_reconnect_folders"),
  /** Closes the player (resume point, last SRAM, play time). */
  exitGame: () => invoke<void>("web_exit_game"),
  /** The slot picker's answer (`null` = cancelled). */
  chooseSlot: (slot: StateSlotId | null) => invoke<void>("web_slot_picker_choose", { slot }),
  /** Data folder paths of SRAM kept in the browser because writing failed. */
  pendingSaves: () => invoke<string[]>("web_pending_saves"),
  /** Writes them again; resolves to the paths still not written. */
  retryPendingSaves: () => invoke<string[]>("web_retry_pending_saves"),
};
