// Which Data folder the web edition uses (Q1/Q2): browser storage (OPFS
// `nostyra-data/`, the default) or a folder the user picked — ideally the desktop
// app's, so both share saves/, config/ and system/. Switching to a folder copies
// the browser's files without overwriting anything there.
import { planCopyWithoutOverwrite } from "./data-folder-copy-plan";
import { DataDir } from "./data-dir";
import { SHARED_ROOTS, looksLikeDesktopDataFolder } from "./data-paths";
import { deleteFolder, getFolder, putFolder } from "./folder-handle-store";

const DATA_FOLDER_ID = "data";
const OPFS_DATA_DIR = "nostyra-data";

export interface DataFolderStatus {
  /** `browser` = OPFS; `folder` = a picked folder. */
  kind: "browser" | "folder";
  /** Picked folder name (the browser never reveals its full path). */
  name: string | null;
  /** The picked folder is a Nostyra desktop Data folder. */
  sharedWithDesktop: boolean;
  /** The picked folder needs a click to grant access again this visit. */
  needsAccess: boolean;
}

export interface SwitchResult {
  copied: number;
  /** Files kept as they are in the folder (not overwritten), relative paths. */
  conflicts: string[];
}

async function browserDir(): Promise<DataDir> {
  const root = await navigator.storage.getDirectory();
  return new DataDir(await root.getDirectoryHandle(OPFS_DATA_DIR, { create: true }));
}

/** The Data folder to read and write now (browser storage when the picked folder
 *  is not accessible this visit: nothing is written into the wrong place silently
 *  — callers check `status().needsAccess` and ask to reconnect first). */
export async function currentDataDir(): Promise<DataDir> {
  const stored = await getFolder(DATA_FOLDER_ID);
  if (stored && (await stored.handle.queryPermission({ mode: "readwrite" })) === "granted") {
    return new DataDir(stored.handle);
  }
  return browserDir();
}

export async function dataFolderStatus(): Promise<DataFolderStatus> {
  const stored = await getFolder(DATA_FOLDER_ID);
  if (!stored) return { kind: "browser", name: null, sharedWithDesktop: false, needsAccess: false };
  const granted = (await stored.handle.queryPermission({ mode: "readwrite" })) === "granted";
  const names = granted ? (await new DataDir(stored.handle).list()).map((entry) => entry.name) : [];
  return {
    kind: "folder",
    name: stored.name,
    sharedWithDesktop: looksLikeDesktopDataFolder(names),
    needsAccess: !granted,
  };
}

/** Shows the folder picker (in a click). `null` = cancelled — which is also what a
 *  folder Chrome refuses (AppData, Program Files…) ends as, after its own dialog. */
export async function pickDataFolder(): Promise<FileSystemDirectoryHandle | null> {
  if (!window.showDirectoryPicker) throw new Error("This browser cannot open folders (use Chrome or Edge).");
  try {
    return await window.showDirectoryPicker({ id: "nostyra-data", mode: "readwrite", startIn: "documents" });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") return null;
    throw error;
  }
}

/** Uses `handle` as the Data folder after copying the browser's shared files into
 *  it without overwriting (the browser copy stays until cleared). */
export async function switchToFolder(handle: FileSystemDirectoryHandle): Promise<SwitchResult> {
  const from = await browserDir();
  const to = new DataDir(handle);
  const source: string[] = [];
  const target: string[] = [];
  for (const root of SHARED_ROOTS) {
    source.push(...(await from.listFilesDeep(root)));
    target.push(...(await to.listFilesDeep(root)));
  }
  const plan = planCopyWithoutOverwrite(source, target);
  for (const path of plan.copy) {
    const file = await from.readFile(path);
    if (file) await to.writeFile(path, file);
  }
  await putFolder({ id: DATA_FOLDER_ID, kind: "data", name: handle.name, handle });
  return { copied: plan.copy.length, conflicts: plan.conflicts };
}

/** Back to browser storage: forgets the folder (its files and the browser copy stay). */
export async function useBrowserStorage(): Promise<void> {
  await deleteFolder(DATA_FOLDER_ID);
}

/** Deletes the browser copy of saves/config/system (only while a folder is used). */
export async function clearBrowserCopy(): Promise<void> {
  if (!(await getFolder(DATA_FOLDER_ID))) throw new Error("Browser storage is the Data folder in use.");
  const root = await navigator.storage.getDirectory();
  const dir = await root.getDirectoryHandle(OPFS_DATA_DIR, { create: true });
  for (const name of SHARED_ROOTS) {
    await dir.removeEntry(name, { recursive: true }).catch(() => undefined);
  }
}
