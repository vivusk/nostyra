// System registry commands (`system_catalog_commands.rs`): every system Nostyra
// knows, and the systems the library has games of.
import { invoke } from "@tauri-apps/api/core";

/** One registry system (labels, extensions, pad actions, core). */
export interface SystemView {
  id: string;
  displayName: string;
  shortLabel: string;
  extensions: string[];
  /** RetroPad actions its pad has, in UI order. */
  padActions: string[];
  /** The system's own button names (Mega Drive `y` → "A"); empty = RetroPad names. */
  padLabels: { action: string; label: string }[];
  coreId: string;
}

/** A system with games in the library. */
export interface LibrarySystem {
  id: string;
  displayName: string;
  shortLabel: string;
  count: number;
}

export const systemCommands = {
  getSystems: () => invoke<SystemView[]>("get_systems"),
  getLibrarySystems: () => invoke<LibrarySystem[]>("get_library_systems"),
};
