// Web backend (nostyra.com/online): composes the per-group command tables, the
// in-page event bus, folder pickers and asset URLs. Later phases add group files,
// not lines here.
import type { Backend } from "../backend-contract";
import { assetToken, resolveAssetToken } from "./artwork/asset-resolver";
import { dataFolderCommands } from "./data-folder";
import { desktopOnlyCommands } from "./desktop-only";
import { gameplayCommands } from "./gameplay";
import { libraryCommands } from "./library";
import { playCommands } from "./play";
import { settingsCommands } from "./settings";
import { commandError, mergeTables } from "./web-command-table";
import { pickFile } from "./fs/picked-files";
import { pickRomFolder } from "./fs/rom-folders";
import { activeGameId } from "./player/active-session";
import { retryPendingSaves } from "./player/pending-saves";
import { holdSingleTabLock } from "./single-tab-lock";
import { listen } from "./web-event-bus";

const commands = mergeTables(
  libraryCommands,
  playCommands,
  gameplayCommands,
  dataFolderCommands,
  settingsCommands,
  desktopOnlyCommands,
);

const backend: Backend = {
  edition: "web",
  boot: async () => {
    if (!(await holdSingleTabLock())) return "busy";
    // SRAM a past session could not write (busy file): try again now (phase 00 D13).
    void retryPendingSaves(activeGameId() !== null).catch((error) => console.warn("pending saves not retried", error));
    return "ready";
  },
  capabilities: {
    coreDownload: false,
    achievements: false,
    shaders: false,
    videoScaling: false,
    runAhead: false,
    rewind: false,
    logs: false,
    revealInFolder: false,
    memoryCards: false,
    gamepadCaptureNative: false,
    dataFolderMove: false,
  },
  invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
    const handler = commands[command];
    if (!handler) throw commandError("unknown_command", `web edition has no command ${command}`);
    return (await handler(args)) as T;
  },
  listen,
  // Folder picker → opaque ref for `add_library_folder`; file picker → opaque ref
  // for `add_bios_file`.
  open: async (options) => (options.directory ? pickRomFolder() : pickFile()),
  assetUrl: assetToken,
  resolveAsset: resolveAssetToken,
};

export default backend;
