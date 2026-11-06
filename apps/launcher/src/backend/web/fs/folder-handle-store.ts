// Folders the user picked (ROM folders, the Data folder), kept across visits:
// File System Access handles are structured-clonable, so they live in IndexedDB.
// Permission is not stored; see folder-permissions.ts.

export type FolderKind = "rom" | "data";

export interface StoredFolder {
  /** ROM folders: the token of `library_folders.path` (`web:<token>`); the Data
   *  folder: `"data"`. */
  id: string;
  kind: FolderKind;
  name: string;
  handle: FileSystemDirectoryHandle;
}

const DB_NAME = "nostyra-online";
const STORE = "folders";

function openStore(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE, { keyPath: "id" });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function run<T>(mode: IDBTransactionMode, action: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await openStore();
  try {
    return await new Promise<T>((resolve, reject) => {
      const request = action(db.transaction(STORE, mode).objectStore(STORE));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  } finally {
    db.close();
  }
}

export function putFolder(folder: StoredFolder): Promise<IDBValidKey> {
  return run("readwrite", (store) => store.put(folder));
}

export async function getFolder(id: string): Promise<StoredFolder | null> {
  return (await run<StoredFolder | undefined>("readonly", (store) => store.get(id))) ?? null;
}

export async function listFolders(kind?: FolderKind): Promise<StoredFolder[]> {
  const all = await run<StoredFolder[]>("readonly", (store) => store.getAll());
  return kind ? all.filter((folder) => folder.kind === kind) : all;
}

export function deleteFolder(id: string): Promise<undefined> {
  return run("readwrite", (store) => store.delete(id));
}
