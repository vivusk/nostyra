// Reads a game's files from its library folder (`games.rom_path` =
// `web:<token>/<relative path>`): the ROM itself and siblings (patches, cue tracks,
// playlist discs). Names match case-insensitively, like the Windows paths the
// desktop resolves.
import { getFolder } from "../fs/folder-handle-store";
import { splitRomPath } from "../scan/scan-paths";
import { commandError } from "../web-command-table";

export interface RomFolder {
  root: FileSystemDirectoryHandle;
  /** Path of the game's main file inside the folder (`a/b/Game.cue`). */
  relative: string;
}

/** The library folder of `romPath`, with read access (asked again when the visit's
 *  permission lapsed: Play is a click). */
export async function openRomFolder(romPath: string): Promise<RomFolder> {
  const parts = splitRomPath(romPath);
  const stored = parts ? await getFolder(parts.token) : null;
  if (!parts || !stored) throw commandError("invalid", "The ROM folder is no longer in the library. Add it again.");
  let state = await stored.handle.queryPermission({ mode: "read" });
  if (state === "prompt") state = await stored.handle.requestPermission({ mode: "read" }).catch(() => state);
  if (state !== "granted") {
    throw commandError("invalid", `Nostyra cannot read the folder ${stored.name} right now. Use “Reconnect” and try again.`);
  }
  return { root: stored.handle, relative: parts.relative };
}

async function entryOf(dir: FileSystemDirectoryHandle, name: string, kind: FileSystemHandleKind) {
  const lower = name.toLowerCase();
  for await (const handle of dir.values()) {
    if (handle.kind === kind && handle.name.toLowerCase() === lower) return handle;
  }
  return null;
}

/** The folder holding `relative` (null when a folder on the way is gone). */
async function parentDir(folder: RomFolder, relative: string): Promise<FileSystemDirectoryHandle | null> {
  let dir: FileSystemDirectoryHandle | null = folder.root;
  for (const part of relative.split("/").slice(0, -1)) {
    if (part === "" || part === ".") continue;
    if (part === "..") return null; // library files never point outside their folder
    dir = (await entryOf(dir, part, "directory")) as FileSystemDirectoryHandle | null;
    if (!dir) return null;
  }
  return dir;
}

/** The file at `relative` inside the folder, or null. */
export async function readRomFile(folder: RomFolder, relative: string): Promise<File | null> {
  const dir = await parentDir(folder, relative);
  const name = relative.slice(relative.lastIndexOf("/") + 1);
  const handle = dir ? ((await entryOf(dir, name, "file")) as FileSystemFileHandle | null) : null;
  return handle ? handle.getFile() : null;
}

/** File names beside `relative` (same folder). */
export async function siblingNames(folder: RomFolder, relative: string): Promise<string[]> {
  const dir = await parentDir(folder, relative);
  const names: string[] = [];
  if (!dir) return names;
  for await (const handle of dir.values()) if (handle.kind === "file") names.push(handle.name);
  return names;
}
