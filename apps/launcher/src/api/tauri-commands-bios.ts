// BIOS and memory card commands (`bios_commands.rs`, `memory_card_commands.rs`).
import { invoke } from "@tauri-apps/api/core";

/** A file in the system folder that looks like a BIOS. */
export interface BiosFile {
  fileName: string;
  /** Console model ("SCPH-1001") when the name or md5 is known. */
  model: string | null;
  region: string;
  /** False: wrong signature or unknown dump, the file is not used. */
  valid: boolean;
}

/** What one region boots: a real BIOS file, or `null` = built-in HLE BIOS. */
export interface BiosRegion {
  region: string;
  /** Row title: the region code, or "All regions" for a BIOS that boots every region. */
  label: string;
  fileName: string | null;
}

/** A core that can use BIOS files (PCSX ReARMed, Flycast). */
export interface CoreBios {
  coreId: string;
  coreName: string;
  systemNames: string[];
  folder: string;
  files: BiosFile[];
  regions: BiosRegion[];
}

/** One memory card slot of a game (1-based). */
export interface MemoryCard {
  slot: number;
  fileName: string;
  /** Saves on the card; null = no readable card (see `unreadable`). */
  saves: number | null;
  /** A file exists but is not a valid memory card. */
  unreadable: boolean;
}

export interface MemoryCardImport {
  saves: number;
  /** A save on the card carries the game's serial; null = the game has no serial. */
  serialFound: boolean | null;
}

export const biosCommands = {
  getBiosStatus: () => invoke<CoreBios[]>("get_bios_status"),
  addBiosFile: (coreId: string, path: string) => invoke<string>("add_bios_file", { coreId, path }),
  openSystemFolder: () => invoke<void>("open_system_folder"),
  getMemoryCards: (gameId: number) => invoke<MemoryCard[]>("get_memory_cards", { gameId }),
  importMemoryCard: (gameId: number, slot: number, path: string) =>
    invoke<MemoryCardImport>("import_memory_card", { gameId, slot, path }),
};
