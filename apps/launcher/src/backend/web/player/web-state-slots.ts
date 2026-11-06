// Web save states in the game's save folder: `web-slot0..9` / `web-auto` with
// `.state` (EmulatorJS's RetroArch-wrapped bytes, opaque), `.json` (the desktop's
// sidecar format, `build: "web"`) and `.png`. Never the desktop's `slotN` / `auto`
// names (plan, user note): each app lists only its own. Names, sidecars and the
// refusal rule come from the shared Rust code through wasm.
import type { SaveStateSlot, StateSlotId } from "../../../api/tauri-commands-gameplay";
import type { DataDir } from "../fs/data-dir";
import { loadWasm } from "../wasm/wasm-module";
import { dataDirRef } from "./data-dir-refs";
import { stateFilePaths, stemArgument } from "./web-file-names";

/** Slots in listing order: the resume point first, then 0–9. */
export const WEB_SLOTS: readonly StateSlotId[] = ["auto", 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

export interface StateSidecarJson {
  core_name: string;
  core_version: string;
  created_at: number;
  sram_crc32?: number;
  rom_patch_crc32?: number;
}

/** `{dir}/web-slot3.state` etc.; throws for a slot outside 0–9 / auto. */
export async function webStateFiles(dir: string, slot: StateSlotId) {
  const stem = (await loadWasm()).web_state_stem(stemArgument(slot));
  if (!stem) throw new Error(`Save slot ${String(slot)} does not exist (0–9 or auto).`);
  return stateFilePaths(dir, stem);
}

function parseSidecar(text: string | null): StateSidecarJson | null {
  try {
    return text ? (JSON.parse(text) as StateSidecarJson) : null;
  } catch {
    return null;
  }
}

/** One slot as `list_save_states` shows it, or null when it holds no state. */
async function readSlot(data: DataDir, dir: string, slot: StateSlotId, ejsCore: string): Promise<SaveStateSlot | null> {
  const files = await webStateFiles(dir, slot);
  const state = await data.readFile(files.state);
  if (!state) return null;
  const text = await data.readText(files.sidecar);
  const sidecar = parseSidecar(text);
  const thumbnail = await data.readFile(files.thumbnail);
  return {
    slot,
    createdAt: sidecar?.created_at ?? Math.floor(state.lastModified / 1000),
    coreName: sidecar?.core_name ?? null,
    coreVersion: sidecar?.core_version ?? null,
    thumbnailPath: thumbnail ? dataDirRef(files.thumbnail) : null,
    sizeBytes: state.size,
    incompatibleReason: text ? ((await loadWasm()).web_state_refusal(text, ejsCore) ?? null) : null,
  };
}

/** Used web slots, resume point first. */
export async function listWebStates(data: DataDir, dir: string, ejsCore: string): Promise<SaveStateSlot[]> {
  const slots = await Promise.all(WEB_SLOTS.map((slot) => readSlot(data, dir, slot, ejsCore)));
  return slots.filter((slot): slot is SaveStateSlot => slot !== null);
}

/** The slot's sidecar (null when missing or unreadable). */
export async function readWebStateSidecar(data: DataDir, dir: string, slot: StateSlotId): Promise<StateSidecarJson | null> {
  return parseSidecar(await data.readText((await webStateFiles(dir, slot)).sidecar));
}

/** A state to load: its bytes, or why it cannot be loaded (empty / refused). */
export async function readWebState(
  data: DataDir,
  dir: string,
  slot: StateSlotId,
  ejsCore: string,
): Promise<{ bytes: Uint8Array } | { refusal: string }> {
  const files = await webStateFiles(dir, slot);
  const state = await data.readFile(files.state);
  const label = slot === "auto" ? "The resume point" : `Slot ${slot}`;
  if (!state) return { refusal: `${label} is empty.` };
  const text = await data.readText(files.sidecar);
  const why = text ? (await loadWasm()).web_state_refusal(text, ejsCore) : undefined;
  if (why) return { refusal: `${label} cannot be loaded: ${why.charAt(0).toLowerCase()}${why.slice(1)}.` };
  return { bytes: new Uint8Array(await state.arrayBuffer()) };
}

export interface NewWebState {
  ejsCore: string;
  state: Uint8Array;
  screenshot: Uint8Array | null;
  sramCrc32: number | null;
  patchCrc32: number | null;
}

/** Writes the slot (state, thumbnail, then sidecar); an old thumbnail goes when
 *  there is no new one. */
export async function writeWebState(data: DataDir, dir: string, slot: StateSlotId, value: NewWebState): Promise<void> {
  const files = await webStateFiles(dir, slot);
  const wasm = await loadWasm();
  const sidecar = wasm.web_state_sidecar(
    value.ejsCore,
    Math.floor(Date.now() / 1000),
    value.sramCrc32,
    value.patchCrc32,
    value.state,
  );
  await data.writeFile(files.state, value.state);
  if (value.screenshot) await data.writeFile(files.thumbnail, value.screenshot);
  else await data.remove(files.thumbnail);
  await data.writeFile(files.sidecar, sidecar);
}

/** Removes the slot's files (state first: the slot stops listing at once). */
export async function deleteWebState(data: DataDir, dir: string, slot: StateSlotId): Promise<void> {
  const files = await webStateFiles(dir, slot);
  for (const path of [files.state, files.sidecar, files.thumbnail]) await data.remove(path);
}
