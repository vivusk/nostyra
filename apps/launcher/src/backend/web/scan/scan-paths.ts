// Names of ROM files on the web: a library folder is `library_folders.path` =
// `web:<token>` (token = random id chosen when the folder was added), a game's
// `games.rom_path` = `web:<token>/<relative path>`, and the wasm module reads the
// file under the virtual path `/web/<token>/<relative path>`. Plus the pure rules
// of the scan (fingerprints, vanished files) mirrored from game-library.

const WEB_PREFIX = "web:";

/** `library_folders.path` of a folder token. */
export function folderPathOf(token: string): string {
  return `${WEB_PREFIX}${token}`;
}

/** Token of a `library_folders.path` (null for anything else). */
export function tokenOfFolderPath(path: string): string | null {
  return path.startsWith(WEB_PREFIX) && !path.includes("/") ? path.slice(WEB_PREFIX.length) : null;
}

/** `games.rom_path` of a file of a folder. */
export function romPathOf(token: string, relative: string): string {
  return `${WEB_PREFIX}${token}/${relative}`;
}

/** `{ token, relative }` of a `games.rom_path`, or null. */
export function splitRomPath(romPath: string): { token: string; relative: string } | null {
  if (!romPath.startsWith(WEB_PREFIX)) return null;
  const rest = romPath.slice(WEB_PREFIX.length);
  const slash = rest.indexOf("/");
  return slash > 0 ? { token: rest.slice(0, slash), relative: rest.slice(slash + 1) } : null;
}

/** Path the wasm module reads a folder's file under. */
export function virtualPathOf(token: string, relative: string): string {
  return `/web/${token}/${relative}`;
}

/** `{ token, relative }` of a virtual path, or null. */
export function splitVirtualPath(path: string): { token: string; relative: string } | null {
  const match = /^\/web\/([^/]+)\/(.+)$/.exec(path);
  return match ? { token: match[1], relative: match[2] } : null;
}

/** Lower-case extension without the dot ("" when none). */
export function extensionOf(name: string): string {
  const base = name.slice(name.lastIndexOf("/") + 1);
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(dot + 1).toLowerCase() : "";
}

/** Single-ROM archive candidate (`rom_identity::is_rom_archive`). */
export function isRomArchive(name: string): boolean {
  return extensionOf(name) === "zip";
}

/** Last scan's facts of a present game row (`file_fingerprints`). The web stores the
 *  File's `lastModified` (ms) in `file_mtime_ns`: a fingerprint, never shown. */
export interface KnownFile {
  size: number;
  mtime: number;
  platformId: string;
}

/** Unchanged since the last scan: not re-hashed. */
export function isUnchanged(known: KnownFile | undefined, size: number, mtime: number): boolean {
  return known !== undefined && known.size === size && known.mtime === mtime;
}

/** Rows to flag missing after a walk (`library_scan_discovery::discover_folder`):
 *  known rows not found again — except archives while ZIP scanning is off that
 *  still exist (opting out pauses discovery, it does not erase games), and files
 *  under an entry the walk could not read (unknown, not vanished). */
export function vanishedPaths(
  known: Iterable<string>,
  present: ReadonlySet<string>,
  existing: ReadonlySet<string>,
  unreadable: readonly string[],
  zipScanning: boolean,
): string[] {
  return [...known].filter(
    (path) =>
      !present.has(path) &&
      (zipScanning || !isRomArchive(path) || !existing.has(path)) &&
      !unreadable.some((bad) => path === bad || path.startsWith(`${bad}/`)),
  );
}

/** Folders the walk never enters: dot-folders and Windows bookkeeping (recycle bin,
 *  System Volume Information) that the browser cannot read anyway. */
export function isSkippedFolder(name: string): boolean {
  const lower = name.toLowerCase();
  return name.startsWith(".") || lower.startsWith("$recycle") || lower === "system volume information";
}
