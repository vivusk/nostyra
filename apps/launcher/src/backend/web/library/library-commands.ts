// Library commands of the web backend: setup state, ROM folders, game list, summary,
// systems and the scan (src-tauri library_commands.rs, scan_commands.rs,
// system_catalog_commands.rs). Same command names and result shapes as the desktop.
import type { GameListQuery, SetupState } from "../../../api/tauri-commands";
import type { LibrarySystem, SystemView } from "../../../api/tauri-commands-systems";
import { isArtworkFetching } from "../artwork/artwork-fetch-job";
import { allSystems, webSystems } from "../catalog/web-systems";
import { libraryPlatformCounts, librarySummary, listGames } from "../db/game-queries";
import { listLibraryFolderRows } from "../db/library-folder-queries";
import { DATA_LOCATION_CHOSEN_SETTING, getSetting } from "../db/settings-queries";
import { dataFolderStatus } from "../fs/data-folder";
import { addRomFolder, libraryFolders, removeRomFolder } from "../fs/rom-folders";
import { activeGameId } from "../player/active-session";
import { cancelScan, currentScanProgress, isScanning, startScan } from "../scan/scan-command";
import { commandError, type CommandTable } from "../web-command-table";

const unixNow = () => Math.floor(Date.now() / 1000);

async function setupState(): Promise<SetupState> {
  const status = await dataFolderStatus().catch(() => null);
  return {
    hasFolders: (await listLibraryFolderRows()).length > 0,
    // Cores ship with the site: nothing to install.
    cores: [],
    datMissing: false,
    dataRoot: status?.name ?? "Browser storage",
    scanning: isScanning(),
    artworkFetching: isArtworkFetching(),
    coreDownloading: false,
    activeGameId: activeGameId(),
    dataFolderChosen: (await getSetting(DATA_LOCATION_CHOSEN_SETTING)) === "1",
  };
}

/** `get_systems` on the web: the systems the browser plays, registry order. */
async function systemViews(): Promise<SystemView[]> {
  return (await webSystems()).map((s) => ({
    id: s.id,
    displayName: s.displayName,
    shortLabel: s.shortLabel,
    extensions: s.extensions,
    padActions: s.padActions,
    padLabels: s.padLabels,
    coreId: s.coreId,
  }));
}

/** `get_library_systems`: registry systems with visible games, display order. */
async function librarySystems(): Promise<LibrarySystem[]> {
  const counts = new Map((await libraryPlatformCounts()).map((c) => [c.platformId, c.count]));
  return (await allSystems())
    .filter((s) => counts.has(s.id))
    .sort((a, b) => a.displayRank - b.displayRank)
    .map((s) => ({ id: s.id, displayName: s.displayName, shortLabel: s.shortLabel, count: counts.get(s.id) ?? 0 }));
}

export const libraryCommands: CommandTable = {
  get_setup_state: () => setupState(),
  list_library_folders: () => libraryFolders(),
  /** `path` = the ref the folder picker returned (`open({ directory: true })`). */
  add_library_folder: ({ path }) => addRomFolder(String(path), unixNow()),
  remove_library_folder: async ({ id }) => {
    // The scan holds a snapshot of the folder list (desktop rule).
    if (isScanning()) throw commandError("scan", "Wait for the scan to finish (or cancel it) before removing a folder.");
    return removeRomFolder(Number(id));
  },
  list_games: ({ query }) => listGames(query as GameListQuery),
  library_summary: () => librarySummary(),
  get_systems: () => systemViews(),
  get_library_systems: () => librarySystems(),
  start_scan: async () => startScan(),
  cancel_scan: async () => cancelScan(),
  get_scan_progress: async () => currentScanProgress(),
};
