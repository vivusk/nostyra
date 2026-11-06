// The web artwork cache in the origin-private file system (OPFS):
// `artwork/<gameId>-cover.jpg`, `artwork/<gameId>-hero.png` and `artwork/index/`
// (thumbnail indexes). Web-only, never the shared Data folder: the desktop keys its
// art by its own DB ids. Writes go through `createWritable` (swap + rename on
// close), so a cancelled job never leaves half a file under a final name.
import { ARTWORK_DIR } from "./artwork-refs";

/** Sub-folder of the thumbnail indexes; survives "Clear cache" (desktop rule). */
const INDEX_DIR = "index";

async function opfsRoot(): Promise<FileSystemDirectoryHandle> {
  return navigator.storage.getDirectory();
}

async function artworkDir(): Promise<FileSystemDirectoryHandle> {
  return (await opfsRoot()).getDirectoryHandle(ARTWORK_DIR, { create: true });
}

async function indexDir(): Promise<FileSystemDirectoryHandle> {
  return (await artworkDir()).getDirectoryHandle(INDEX_DIR, { create: true });
}

async function writeInto(dir: FileSystemDirectoryHandle, name: string, data: Blob | string): Promise<void> {
  const handle = await dir.getFileHandle(name, { create: true });
  const writable = await handle.createWritable({ keepExistingData: false });
  try {
    await writable.write(data);
    await writable.close();
  } catch (error) {
    await writable.abort().catch(() => undefined);
    throw error;
  }
}

/** Stores a cached art file (`<gameId>-cover.jpg` …). */
export async function writeArtworkFile(name: string, data: Blob): Promise<void> {
  await writeInto(await artworkDir(), name, data);
}

/** Deletes cached art files of a game (missing-game removal); absent ones are fine. */
export async function removeArtworkFiles(names: string[]): Promise<void> {
  const dir = await artworkDir();
  for (const name of names) await dir.removeEntry(name).catch(() => undefined);
}

/** A file of OPFS by path parts, or null. */
export async function readOpfsFile(parts: string[]): Promise<File | null> {
  try {
    let dir = await opfsRoot();
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
    return await (await dir.getFileHandle(parts[parts.length - 1])).getFile();
  } catch {
    return null;
  }
}

/** `artwork_cache_size`: cached art files (indexes excluded). */
export async function artworkCacheStats(): Promise<{ files: number; bytes: number }> {
  const stats = { files: 0, bytes: 0 };
  for await (const handle of (await artworkDir()).values()) {
    if (handle.kind !== "file") continue;
    stats.files += 1;
    stats.bytes += (await (handle as FileSystemFileHandle).getFile()).size;
  }
  return stats;
}

/** `clear_artwork_cache`: deletes every cached art file, keeps the indexes. */
export async function clearArtworkFiles(): Promise<number> {
  const dir = await artworkDir();
  const names: string[] = [];
  for await (const handle of dir.values()) if (handle.kind === "file") names.push(handle.name);
  for (const name of names) await dir.removeEntry(name);
  return names.length;
}

/** A cached thumbnail index (`index/<system>.json`), or null. */
export async function readIndexFile(name: string): Promise<string | null> {
  try {
    return await (await (await indexDir()).getFileHandle(name)).getFile().then((file) => file.text());
  } catch {
    return null;
  }
}

export async function writeIndexFile(name: string, text: string): Promise<void> {
  await writeInto(await indexDir(), name, text);
}
