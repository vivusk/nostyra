// Friendly names of the save files a cloud conflict lists ("Battery save", "Slot 3",
// "Resume point", "Memory card"). Paths are unit-relative and `/`-separated, as the
// backend reports them (`save.srm`, `slot3.state`, `PSP/SAVEDATA/ULUS10041/DATA.BIN`).

/** Web edition stem prefix of state files synced from the browser (`web-slot1.state`). */
const WEB_STATE_PREFIX = "web-";
const STATE_EXTS = new Set(["state", "json", "png"]);
const BATTERY_EXTS = new Set(["srm", "sav", "eep", "fla", "mpk", "sra", "dsv"]);
const MEMORY_CARD_EXTS = new Set(["ps2", "mcd", "mcr", "mc", "raw", "gci"]);

/** Label of one conflicted file. */
export function cloudFileLabel(path: string): string {
  const parts = path.split("/").filter(Boolean);
  const name = parts[parts.length - 1] ?? path;
  if (parts.length > 1) return folderLabel(parts);
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  let stem = dot > 0 ? name.slice(0, dot).toLowerCase() : name.toLowerCase();
  // `save.web.srm`, `save.srm` and the like: the extension decides.
  if (BATTERY_EXTS.has(ext)) return "Battery save";
  if (MEMORY_CARD_EXTS.has(ext)) return "Memory card";
  if (STATE_EXTS.has(ext)) {
    if (stem.startsWith(WEB_STATE_PREFIX)) stem = stem.slice(WEB_STATE_PREFIX.length);
    if (stem === "auto") return "Resume point";
    const slot = /^slot(\d+)$/.exec(stem);
    if (slot) return `Slot ${Number(slot[1])}`;
  }
  return name;
}

/** Saves inside a folder: PSP `SAVEDATA/<id>`, Dolphin / melonDS card folders. */
function folderLabel(parts: string[]): string {
  const upper = parts.map((part) => part.toUpperCase());
  const saveData = upper.indexOf("SAVEDATA");
  if (saveData >= 0 && parts[saveData + 1]) return `Save data ${parts[saveData + 1]}`;
  if (upper.some((part) => part.startsWith("CARD") || part === "GC")) return "Memory card";
  return "Save data";
}

/** Labels of a conflict's files, without duplicates (a slot's .state / .png / .json
 *  pair shows once), in the backend's order. */
export function cloudFileLabels(paths: readonly string[]): string[] {
  return [...new Set(paths.map(cloudFileLabel))];
}
