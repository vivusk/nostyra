// ROM folders on the web: the directory picker hands out an opaque ref
// (`web-pick:<uuid>`, the handle parked in memory until `add_library_folder` takes
// it), the folder becomes a `library_folders` row `web:<token>` and its handle is
// kept in IndexedDB under the token. The UI shows the folder's name where the
// desktop shows its path.
import type { LibraryFolder } from "../../../api/tauri-commands";
import { addLibraryFolderRow, listLibraryFolderRows, removeLibraryFolderRow } from "../db/library-folder-queries";
import { folderPathOf, tokenOfFolderPath } from "../scan/scan-paths";
import { deleteFolder, listFolders, putFolder, type StoredFolder } from "./folder-handle-store";

const PICK_REF = "web-pick:";
const picked = new Map<string, FileSystemDirectoryHandle>();

/** Shows the folder picker (read access); the ref `add_library_folder` accepts, or
 *  null when cancelled. */
export async function pickRomFolder(): Promise<string | null> {
  if (!window.showDirectoryPicker) throw new Error("This browser cannot open folders. Use Chrome or Edge.");
  try {
    const handle = await window.showDirectoryPicker({ mode: "read", id: "nostyra-roms" });
    const ref = `${PICK_REF}${crypto.randomUUID()}`;
    picked.set(ref, handle);
    return ref;
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") return null;
    throw error;
  }
}

/** Short random token naming a folder in `rom_path`s (never reused, unlike row ids). */
function newToken(): string {
  return crypto.randomUUID().replaceAll("-", "").slice(0, 8);
}

/** Adds a picked folder; picking a folder already in the library returns its row
 *  (idempotent like the desktop). */
export async function addRomFolder(ref: string, now: number): Promise<LibraryFolder> {
  const handle = picked.get(ref);
  if (!handle) throw new Error("Choose the folder again.");
  picked.delete(ref);
  for (const stored of await listFolders("rom")) {
    if (await stored.handle.isSameEntry(handle)) {
      const row = (await listLibraryFolderRows()).find((r) => r.path === folderPathOf(stored.id));
      if (row) return { ...row, path: stored.name };
    }
  }
  const token = newToken();
  await putFolder({ id: token, kind: "rom", name: handle.name, handle });
  const row = await addLibraryFolderRow(folderPathOf(token), now);
  return { ...row, path: handle.name };
}

/** A library folder with its handle (null when the handle was lost). */
export interface RomFolder {
  id: number;
  token: string;
  name: string;
  addedAt: number;
  stored: StoredFolder | null;
}

/** Every library folder, oldest first. */
export async function listRomFolders(): Promise<RomFolder[]> {
  const stored = new Map((await listFolders("rom")).map((folder) => [folder.id, folder]));
  return (await listLibraryFolderRows()).map((row) => {
    const token = tokenOfFolderPath(row.path) ?? row.path;
    const folder = stored.get(token) ?? null;
    return { id: row.id, token, name: folder?.name ?? "(folder not available)", addedAt: row.addedAt, stored: folder };
  });
}

/** `list_library_folders` as the UI shows it. */
export async function libraryFolders(): Promise<LibraryFolder[]> {
  return (await listRomFolders()).map((folder) => ({ id: folder.id, path: folder.name, addedAt: folder.addedAt }));
}

/** Removes the row (games hidden, history kept) and forgets the handle. */
export async function removeRomFolder(id: number): Promise<boolean> {
  const folder = (await listRomFolders()).find((f) => f.id === id);
  const removed = await removeLibraryFolderRow(id);
  if (folder) await deleteFolder(folder.token);
  return removed;
}
