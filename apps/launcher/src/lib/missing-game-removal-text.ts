import { edition } from "../backend";

/** What removing a missing game deletes (Library bulk + Detail confirm text). The web
 *  edition removes only its own library rows: saves and settings live in the shared
 *  Data folder (maybe the desktop app's) and are kept. */
export const MISSING_REMOVAL_WARNING =
  edition === "web"
    ? "Removes its play time, favorite, edited metadata and cached artwork from Nostyra Online. " +
      "In-game saves, save states and per-game settings in your Data folder are kept. The ROM file is already gone."
    : "Removes its play time, favorite, edited metadata, cached artwork, in-game saves, save states and " +
      "per-game settings from Nostyra. The ROM file is already gone. This cannot be undone.";
