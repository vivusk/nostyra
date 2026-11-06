// Moving from browser storage to a real Data folder (Q2): every shared file is
// copied unless the target already has it — existing files are never overwritten
// (the desktop's saves win) and are listed as conflicts. Pure planner.

export interface CopyPlan {
  /** Relative paths to copy. */
  copy: string[];
  /** Relative paths present on both sides (kept as they are in the target). */
  conflicts: string[];
}

/** Plans the copy of `source` files into a folder that already has `target` files. */
export function planCopyWithoutOverwrite(source: readonly string[], target: readonly string[]): CopyPlan {
  const existing = new Set(target);
  const copy: string[] = [];
  const conflicts: string[] = [];
  for (const path of [...source].sort()) {
    (existing.has(path) ? conflicts : copy).push(path);
  }
  return { copy, conflicts };
}
