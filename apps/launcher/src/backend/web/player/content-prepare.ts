// The content File the player hands EmulatorJS, prepared like the desktop runner
// does: a cartridge as is, the ROM inside a single-ROM ZIP (wasm `read_zip_rom`,
// desktop rules), soft patches beside the ROM applied in order (wasm `apply_patch`;
// a failing chain starts the game unpatched, never half-patched); a PS1 cue sheet
// with one track as its `.bin` (phase 00 D10: no archive copy), with more tracks as
// a store-only zip of the sheet and its files; an m3u as its first disc; CHD and
// other images directly. Every file is named `game.<ext>` (player-messages.ts).
import type { WebSystem } from "../catalog/web-systems";
import { extensionOf } from "../scan/scan-paths";
import { loadWasm } from "../wasm/wasm-module";
import { commandError } from "../web-command-table";
import { crc32OfBlob } from "./crc32-stream";
import { cueFiles, firstPlaylistEntry, patchChain, siblingPath } from "./content-names";
import { CONTENT_STEM } from "./player-messages";
import { readRomFile, siblingNames, type RomFolder } from "./rom-files";
import { storeZip, type ZipPart } from "./store-zip";

export interface PreparedContent {
  rom: File;
  /** CRC32 of the patch chain found beside the ROM, applied or not (sidecars). */
  patchCrc32: number | null;
}

const missing = (what: string) => commandError("invalid", `${what} was not found. Rescan your library.`);
const named = (data: Blob | Uint8Array, ext: string) =>
  new File([data as BlobPart], `${CONTENT_STEM}.${ext.toLowerCase()}`);

async function mustRead(folder: RomFolder, relative: string): Promise<File> {
  const file = await readRomFile(folder, relative);
  if (!file) throw missing(relative === folder.relative ? "The ROM file" : `${relative}`);
  return file;
}

/** The game's content, ready for the player. */
export async function prepareContent(folder: RomFolder, system: WebSystem): Promise<PreparedContent> {
  const main = await mustRead(folder, folder.relative);
  if (system.media === "disc") return { rom: await discContent(folder, folder.relative, main, 0), patchCrc32: null };
  return cartridgeContent(folder, main);
}

async function cartridgeContent(folder: RomFolder, main: File): Promise<PreparedContent> {
  const wasm = await loadWasm();
  let bytes: Uint8Array | null = null;
  let ext = extensionOf(main.name);
  if (ext === "zip") {
    wasm.files_clear();
    try {
      wasm.files_add_bytes(`/content/${main.name}`, new Uint8Array(await main.arrayBuffer()));
      const rom = wasm.read_zip_rom(`/content/${main.name}`);
      ext = extensionOf(rom.name);
      bytes = rom.take_bytes();
      rom.free();
    } catch (error) {
      throw commandError("rom", `Cannot open ${main.name}: ${String((error as Error).message ?? error)}`);
    } finally {
      wasm.files_clear();
    }
  }
  const { patches, patchCrc32 } = await readPatchChain(folder);
  if (patches.length === 0) return { rom: named(bytes ?? main, ext), patchCrc32: null };
  let image = bytes ?? new Uint8Array(await main.arrayBuffer());
  try {
    for (const patch of patches) image = wasm.apply_patch(image, patch);
  } catch (error) {
    console.warn("patch chain not applied; starting unpatched", error);
    image = bytes ?? new Uint8Array(await main.arrayBuffer());
  }
  return { rom: named(image, ext), patchCrc32 };
}

/** The soft patches beside the game's main file, in chain order, and their CRC
 *  (`rom_patch::patch_chain_crc32`: all patch bytes concatenated; null = none). */
export async function readPatchChain(folder: RomFolder): Promise<{ patches: Uint8Array[]; patchCrc32: number | null }> {
  const main = folder.relative.slice(folder.relative.lastIndexOf("/") + 1);
  const patches: Uint8Array[] = [];
  for (const name of patchChain(main, await siblingNames(folder, folder.relative))) {
    patches.push(new Uint8Array(await (await mustRead(folder, siblingPath(folder.relative, name))).arrayBuffer()));
  }
  if (patches.length === 0) return { patches, patchCrc32: null };
  return { patches, patchCrc32: (await loadWasm()).crc32(concat(patches)) };
}

/** A disc image as EmulatorJS loads it (`depth` guards against playlists of playlists). */
async function discContent(folder: RomFolder, relative: string, file: File, depth: number): Promise<File> {
  const ext = extensionOf(relative);
  if (ext === "m3u" && depth === 0) {
    const first = firstPlaylistEntry(await file.text());
    if (!first) throw commandError("rom", `${file.name} lists no disc.`);
    const disc = siblingPath(relative, first);
    return discContent(folder, disc, await mustRead(folder, disc), depth + 1);
  }
  if (ext !== "cue") return named(file, ext);
  const text = await file.text();
  const tracks = cueFiles(text);
  const files = await Promise.all(tracks.map((name) => mustRead(folder, siblingPath(relative, name))));
  if (files.length === 1 && (text.match(/^\s*TRACK\s/gim) ?? []).length === 1) {
    return named(files[0], extensionOf(tracks[0]));
  }
  const parts: ZipPart[] = [{ name: `${CONTENT_STEM}.${ext}`, data: file }];
  tracks.forEach((name, index) => parts.push({ name: name.replaceAll("\\", "/").split("/").pop() ?? name, data: files[index] }));
  return new File([await storeZip(parts, crc32OfBlob)], `${CONTENT_STEM}.zip`);
}

function concat(parts: readonly Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}
