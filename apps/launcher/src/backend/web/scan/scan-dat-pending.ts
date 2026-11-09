// DAT-gated loose files on the web (Atari 2600 / 7800 `.bin`, M17; desktop
// game-library `dat_pending_resolution`). Pass A: after disc grouping hid the sheet
// tracks, a surviving problem whose extension + size fit a DAT-gated system becomes
// *pending* instead of a problem. Pass B (scan-hashing.ts) reads it and asks each
// candidate's DAT (wasm `dat_hit`); a miss is the old problem with the desktop hint.
// No wasm import here, so the split stays unit-testable under node.

/** A problem of pass A (relative path + reason). */
export interface PassAProblem {
  relative: string;
  reason: string;
}

/** A loose file only a DAT can place. */
export interface PendingFile {
  relative: string;
  /** The classifier's reason ("not a PlayStation disc"), kept for a miss. */
  reason: string;
  /** System ids whose DAT may list it, in registry order (wasm `dat_pending_candidates`). */
  candidates: string[];
}

/**
 * Splits the problems that survived disc grouping: those with DAT candidates are
 * pending (not reported, not "unreadable"), the rest stay problems. Archives are never
 * pending (a zipped `.bin` keeps its pre-M17 result).
 */
export function splitDatPending(
  problems: PassAProblem[],
  candidatesOf: (relative: string) => string[],
  isArchive: (relative: string) => boolean,
): { problems: PassAProblem[]; pending: PendingFile[] } {
  const kept: PassAProblem[] = [];
  const pending: PendingFile[] = [];
  for (const problem of problems) {
    const candidates = isArchive(problem.relative) ? [] : candidatesOf(problem.relative);
    if (candidates.length > 0) pending.push({ ...problem, candidates });
    else kept.push(problem);
  }
  return { problems: kept, pending };
}
