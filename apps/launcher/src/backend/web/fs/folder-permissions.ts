// Chrome grants folder access per visit unless the user chose "Allow on every
// visit": a stored handle comes back as `prompt`, and only a click may ask again.
// The launcher shows one "Reconnect folders" banner that re-asks for all of them.
import { listFolders, type StoredFolder } from "./folder-handle-store";

export type AccessMode = "read" | "readwrite";

/** Access mode each folder kind needs. */
export function modeFor(folder: StoredFolder): AccessMode {
  return folder.kind === "data" ? "readwrite" : "read";
}

/** Current permission without prompting. */
export async function permissionOf(handle: FileSystemHandle, mode: AccessMode): Promise<PermissionState> {
  return handle.queryPermission({ mode });
}

/** Folders whose permission is not granted right now (need a click). */
export async function foldersNeedingAccess(): Promise<StoredFolder[]> {
  const folders = await listFolders();
  const states = await Promise.all(folders.map((folder) => permissionOf(folder.handle, modeFor(folder))));
  return folders.filter((_, index) => states[index] !== "granted");
}

/** Asks again for every folder (must run inside a user click); returns how many
 *  are still not granted. */
export async function reconnectFolders(): Promise<number> {
  let missing = 0;
  for (const folder of await foldersNeedingAccess()) {
    try {
      const state = await folder.handle.requestPermission({ mode: modeFor(folder) });
      if (state !== "granted") missing += 1;
    } catch {
      missing += 1;
    }
  }
  return missing;
}
