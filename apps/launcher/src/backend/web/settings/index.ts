// Launcher settings kept in the web library DB (phase 04 settings store). Changing
// ZIP scanning starts (or queues) a rescan, as on the desktop.
import {
  ARTWORK_AUTO_FETCH_SETTING,
  ZIP_SCAN_SETTING,
  artworkAutoFetch,
  setSetting,
  zipScanning,
} from "../db/settings-queries";
import { startScan } from "../scan/scan-command";
import type { CommandTable } from "../web-command-table";

export const settingsCommands: CommandTable = {
  get_artwork_auto_fetch: () => artworkAutoFetch(),
  set_artwork_auto_fetch: ({ enabled }) => setSetting(ARTWORK_AUTO_FETCH_SETTING, enabled ? "1" : "0"),
  get_scan_zip_archives: () => zipScanning(),
  set_scan_zip_archives: async ({ enabled }) => {
    await setSetting(ZIP_SCAN_SETTING, enabled ? "1" : "0");
    startScan();
  },
};
