// Walks one library folder through its File System Access handle: every file below
// it (relative path + `File` for size, `lastModified` and reads), folders the
// browser cannot read reported per entry (like walkdir errors on the desktop) — the
// walk goes on. Only the root failing (no permission, folder gone) fails the folder.
import { isSkippedFolder } from "./scan-paths";

export interface WalkedFile {
  relative: string;
  file: File;
}

export interface WalkProblem {
  relative: string;
  reason: string;
}

export interface WalkResult {
  files: WalkedFile[];
  failures: WalkProblem[];
}

/** Every file below `root`. Throws only when `root` itself cannot be listed. */
export async function walkFolder(root: FileSystemDirectoryHandle): Promise<WalkResult> {
  const result: WalkResult = { files: [], failures: [] };
  await walkDirectory(root, "", result, true);
  return result;
}

async function walkDirectory(
  dir: FileSystemDirectoryHandle,
  prefix: string,
  result: WalkResult,
  isRoot: boolean,
): Promise<void> {
  const files: Promise<void>[] = [];
  const folders: { handle: FileSystemDirectoryHandle; relative: string }[] = [];
  try {
    for await (const handle of dir.values()) {
      const relative = prefix ? `${prefix}/${handle.name}` : handle.name;
      if (handle.kind === "directory") {
        if (!isSkippedFolder(handle.name)) {
          folders.push({ handle: handle as FileSystemDirectoryHandle, relative });
        }
        continue;
      }
      // getFile() calls run in parallel: one round-trip per file dominates a rescan.
      files.push(
        (handle as FileSystemFileHandle).getFile().then(
          (file) => void result.files.push({ relative, file }),
          (error: unknown) => void result.failures.push({ relative, reason: reasonOf(error) }),
        ),
      );
    }
  } catch (error) {
    if (isRoot) throw error;
    result.failures.push({ relative: prefix, reason: reasonOf(error) });
  }
  await Promise.all(files);
  for (const folder of folders) await walkDirectory(folder.handle, folder.relative, result, false);
}

/** Readable reason of a File System Access error. */
export function reasonOf(error: unknown): string {
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError") return "no permission to read it (reconnect the folder)";
    if (error.name === "NotFoundError") return "not found";
    return error.message || error.name;
  }
  return error instanceof Error ? error.message : String(error);
}
