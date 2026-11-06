// Web-only commands: the Data folder (browser storage or a picked folder) and
// reconnecting folders whose permission lapsed. Called by api/web-commands.ts.
import {
  clearBrowserCopy,
  dataFolderStatus,
  pickDataFolder,
  switchToFolder,
  useBrowserStorage,
} from "../fs/data-folder";
import { foldersNeedingAccess, reconnectFolders } from "../fs/folder-permissions";
import { DATA_LOCATION_CHOSEN_SETTING, setSetting } from "../db/settings-queries";
import type { CommandTable } from "../web-command-table";

/** Either answer ends the first-run Data folder step. */
const markChosen = () => setSetting(DATA_LOCATION_CHOSEN_SETTING, "1");

export const dataFolderCommands: CommandTable = {
  web_data_folder_status: () => dataFolderStatus(),
  /** Picker → copy without overwrite → use it; `null` when cancelled. */
  web_choose_data_folder: async () => {
    const handle = await pickDataFolder();
    if (!handle) return null;
    const result = await switchToFolder(handle);
    await markChosen();
    return result;
  },
  web_use_browser_storage: async () => {
    await useBrowserStorage();
    await markChosen();
  },
  web_clear_browser_copy: () => clearBrowserCopy(),
  web_folders_needing_access: async () => (await foldersNeedingAccess()).map((folder) => folder.name),
  /** Re-asks for every folder (in a click); resolves to how many are still closed. */
  web_reconnect_folders: () => reconnectFolders(),
};
