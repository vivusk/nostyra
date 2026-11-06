// Pure naming steps of the web player around the shared Rust names (passed in from
// the wasm module, so `node --test` checks them against the real rules): a game's
// entry in a Data folder listing, and the three files of a web save-state slot.
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";

/** wasm `find_entry_name` / `new_entry_name`. */
export interface EntryNaming {
  find(names: string[], sha1: string, ext: string): string | undefined;
  create(romName: string, sha1: string, ext: string): string | undefined;
}

/** The game's entry among `names` (one folder listing): the existing ` [ID]` entry
 *  first (also a desktop-made or renamed one), else the name a new one gets; null
 *  when `sha1` is not a usable content sha1. */
export function entryNameFor(names: string[], sha1: string, romFileName: string, ext: string, naming: EntryNaming): string | null {
  return naming.find(names, sha1, ext) ?? naming.create(romFileName, sha1, ext) ?? null;
}

/** wasm `web_state_stem` argument of a slot (-1 = the resume point); throws for a
 *  slot that does not exist (the desktop's wording). */
export function stemArgument(slot: StateSlotId): number {
  if (slot === "auto") return -1;
  if (!Number.isInteger(slot) || slot < 0 || slot > 9) {
    throw new Error(`Save slot ${String(slot)} does not exist (0–9 or auto).`);
  }
  return slot;
}

/** `{dir}/{stem}.state|.json|.png`. */
export function stateFilePaths(dir: string, stem: string) {
  return { state: `${dir}/${stem}.state`, sidecar: `${dir}/${stem}.json`, thumbnail: `${dir}/${stem}.png` };
}
