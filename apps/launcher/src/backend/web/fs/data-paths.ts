// Relative paths inside the Data folder (same layout as the desktop data root).
// Pure: no browser API, unit-tested with `node --test`.

/** Top-level entries the web edition reads and writes in a shared Data folder. */
export const SHARED_ROOTS = ["saves", "config", "system"] as const;
/** Top-level entries the web never touches (desktop-only or the desktop DB). */
export const DESKTOP_ONLY_ROOTS = ["nostyra.db", "cores", "logs", "achievements", "temp", "cache"] as const;
/** Desktop files that mark a folder as a Nostyra desktop Data folder. */
export const DESKTOP_MARKERS = ["nostyra.db", "config"] as const;

/** Splits `a/b/c.txt` into its segments; rejects empty, `.`, `..`, absolute and
 *  backslash paths (the web only ever builds these from known names). */
export function splitDataPath(path: string): string[] {
  if (path.startsWith("/") || path.includes("\\")) throw new Error(`bad data path: ${path}`);
  const parts = path.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new Error(`bad data path: ${path}`);
  }
  return parts;
}

/** Where the web leaves play sessions for the desktop to import (plan Q5). */
export const PENDING_SESSIONS_DIR = "sessions/pending";

/** Whether the web may write `path`: below the shared roots, or a pending session. */
export function isWritableDataPath(path: string): boolean {
  const [root] = splitDataPath(path);
  return (SHARED_ROOTS as readonly string[]).includes(root) || path.startsWith(`${PENDING_SESSIONS_DIR}/`);
}

/** Whether a folder listing looks like a desktop Data folder. */
export function looksLikeDesktopDataFolder(topLevelNames: readonly string[]): boolean {
  return DESKTOP_MARKERS.some((marker) => topLevelNames.includes(marker));
}
