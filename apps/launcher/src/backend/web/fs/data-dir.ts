// The Data folder as the web edition sees it: browser storage (OPFS
// `nostyra-data/`) or a folder the user picked (maybe the desktop's). Both are
// directory handles, so one implementation serves both. Writes go through
// `createWritable` (swap file + rename on close = atomic) and only below the
// shared roots (saves/, config/, system/) and sessions/pending/ (play time for the
// desktop).
import { isWritableDataPath, splitDataPath } from "./data-paths";

export class DataDir {
  constructor(readonly root: FileSystemDirectoryHandle) {}

  private async dir(parts: string[], create: boolean): Promise<FileSystemDirectoryHandle | null> {
    let current = this.root;
    for (const part of parts) {
      try {
        current = await current.getDirectoryHandle(part, { create });
      } catch (error) {
        if (!create && isNotFound(error)) return null;
        throw error;
      }
    }
    return current;
  }

  private async file(path: string, create: boolean): Promise<FileSystemFileHandle | null> {
    const parts = splitDataPath(path);
    const dir = await this.dir(parts.slice(0, -1), create);
    if (!dir) return null;
    try {
      return await dir.getFileHandle(parts[parts.length - 1], { create });
    } catch (error) {
      if (!create && isNotFound(error)) return null;
      throw error;
    }
  }

  /** The file, or null when it does not exist. */
  async readFile(path: string): Promise<File | null> {
    return (await (await this.file(path, false))?.getFile()) ?? null;
  }

  async readText(path: string): Promise<string | null> {
    return (await (await this.readFile(path))?.text()) ?? null;
  }

  async exists(path: string): Promise<boolean> {
    return (await this.file(path, false)) !== null;
  }

  /** Replaces `path` atomically (folders created as needed). */
  async writeFile(path: string, data: Uint8Array | BufferSource | Blob | string): Promise<void> {
    if (!isWritableDataPath(path)) throw new Error(`the web edition does not write ${path}`);
    const handle = await this.file(path, true);
    if (!handle) throw new Error(`cannot create ${path}`);
    const writable = await handle.createWritable({ keepExistingData: false });
    try {
      await writable.write(data as FileSystemWriteChunkType);
      await writable.close();
    } catch (error) {
      await writable.abort().catch(() => undefined);
      throw error;
    }
  }

  async remove(path: string): Promise<void> {
    if (!isWritableDataPath(path)) throw new Error(`the web edition does not delete ${path}`);
    const parts = splitDataPath(path);
    const dir = await this.dir(parts.slice(0, -1), false);
    await dir?.removeEntry(parts[parts.length - 1]).catch((error) => {
      if (!isNotFound(error)) throw error;
    });
  }

  /** Entry names of the folder at `path` ("" = the root); empty when missing. */
  async list(path = ""): Promise<{ name: string; kind: FileSystemHandleKind }[]> {
    const dir = path === "" ? this.root : await this.dir(splitDataPath(path), false);
    const entries: { name: string; kind: FileSystemHandleKind }[] = [];
    if (!dir) return entries;
    for await (const handle of dir.values()) entries.push({ name: handle.name, kind: handle.kind });
    return entries;
  }

  /** Every file below `path`, as relative paths from the Data folder root. */
  async listFilesDeep(path: string): Promise<string[]> {
    const files: string[] = [];
    for (const entry of await this.list(path)) {
      const child = `${path}/${entry.name}`;
      if (entry.kind === "file") files.push(child);
      else files.push(...(await this.listFilesDeep(child)));
    }
    return files;
  }
}

function isNotFound(error: unknown): boolean {
  return error instanceof DOMException && (error.name === "NotFoundError" || error.name === "TypeMismatchError");
}
