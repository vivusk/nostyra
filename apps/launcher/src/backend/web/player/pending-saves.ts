// SRAM the player could not write to the Data folder (phase 00 D13: the file was
// busy — `InvalidStateError` / `NoModificationAllowedError` — or access was lost)
// is never dropped: at the end of the session it is parked in browser storage
// (OPFS `pending-saves/<data path>`), the launcher shows a banner with Retry, and
// the next start retries on its own. A retry writes the parked bytes over the
// Data folder file (last write wins, plan Q4); a successful session write removes the parked copy.
import { DataDir } from "../fs/data-dir";
import { dataFolderStatus, currentDataDir } from "../fs/data-folder";

const PENDING_DIR = "pending-saves";

async function pendingDir(): Promise<DataDir> {
  const root = await navigator.storage.getDirectory();
  return new DataDir(await root.getDirectoryHandle(PENDING_DIR, { create: true }));
}

/** Parks `bytes` meant for the Data folder file `path` (`saves/…`). */
export async function parkPendingSave(path: string, bytes: Uint8Array): Promise<void> {
  await (await pendingDir()).writeFile(path, new Uint8Array(bytes));
}

/** Drops the parked copy of `path`: a later successful write made it stale. */
export async function removePendingSave(path: string): Promise<void> {
  await (await pendingDir()).remove(path);
}

/** Data folder paths with parked bytes. */
export async function pendingSavePaths(): Promise<string[]> {
  return (await pendingDir()).listFilesDeep("saves");
}

/** Writes every parked save to the Data folder; returns the paths still parked.
 *  Does nothing while a game runs: the session may be writing the same file. */
export async function retryPendingSaves(sessionActive: boolean): Promise<string[]> {
  const pending = await pendingDir();
  const paths = await pending.listFilesDeep("saves");
  if (sessionActive) return paths;
  // Never into browser storage while the chosen folder waits for "Reconnect".
  if (paths.length === 0 || (await dataFolderStatus()).needsAccess) return paths;
  const data = await currentDataDir();
  const left: string[] = [];
  for (const path of paths) {
    const file = await pending.readFile(path);
    try {
      if (file) await data.writeFile(path, file);
      await pending.remove(path);
    } catch (error) {
      console.warn(`pending save ${path} still not written`, error);
      left.push(path);
    }
  }
  return left;
}
