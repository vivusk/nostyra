// The system registry as the web edition sees it (`system_catalog_json` from the
// wasm module, crates/web-bindings catalog_json.rs): one source for labels,
// extensions, the EmulatorJS core, DAT / metadat / thumbnail names.
import { loadWasm } from "../wasm/wasm-module";

export interface WebSystem {
  id: string;
  displayName: string;
  shortLabel: string;
  extensions: string[];
  padActions: string[];
  padLabels: { action: string; label: string }[];
  coreId: string;
  /** Display name of the desktop core (Settings → BIOS). */
  coreName: string;
  media: "cartridge" | "disc";
  /** EmulatorJS core; null = the system needs the desktop app. */
  webCore: { ejsCore: string; saveDir: string } | null;
  coreLayerId: string | null;
  /** BIOS files the system's core can use (desktop core list; empty = none). */
  firmware: FirmwareFile[];
  datFile: string;
  metadatFile: string;
  thumbnailsRepo: string;
  /** Position in the Library system filter. */
  displayRank: number;
  /** Play online support of the desktop app for this system (the browser has none). */
  desktopNetplay: "yes" | "experimental" | "no";
  /** Largest desktop netplay room (players). */
  desktopNetplayPlayers: number;
}

export interface FirmwareFile {
  name: string;
  /** Console model ("SCPH-5501"). */
  model: string;
  md5: string;
  /** "JP" / "US" / "EU", or "ANY" (boots every region). */
  region: string;
  size: number;
  /** Folder below `system/` ("" = `system/` itself). */
  subdir: string;
}

let systems: Promise<WebSystem[]> | null = null;

/** Every registry system, registry order. */
export function allSystems(): Promise<WebSystem[]> {
  systems ??= loadWasm().then((module) => JSON.parse(module.system_catalog_json()) as WebSystem[]);
  return systems;
}

/** Systems the browser can play (`webCore` set). */
export async function webSystems(): Promise<WebSystem[]> {
  return (await allSystems()).filter((system) => system.webCore !== null);
}

/** One system by id, or null. */
export async function systemById(id: string): Promise<WebSystem | null> {
  return (await allSystems()).find((system) => system.id === id) ?? null;
}
