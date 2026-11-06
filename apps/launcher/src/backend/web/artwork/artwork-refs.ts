// Asset refs of the web edition: what `Game.coverPath` / `heroPath` carry instead of
// absolute file paths. `opfs:<path>` names a file in the origin-private file system
// (artwork cache: `opfs:artwork/<gameId>-cover.jpg`); `web/index.ts` turns refs into
// `nostyra-asset:` tokens (`assetUrl`) and those into object URLs (`resolveAsset`).

const OPFS_REF = "opfs:";

/** OPFS folder of the artwork cache (web-only: desktop keys art by its own DB ids). */
export const ARTWORK_DIR = "artwork";

/** Ref of a cached art file (`artwork.local_path`, relative to the cache folder). */
export function artworkRef(localPath: string | null): string | null {
  return localPath ? `${OPFS_REF}${ARTWORK_DIR}/${localPath}` : null;
}

/** OPFS path parts of a ref, or null when it is not an OPFS ref. */
export function opfsPartsOfRef(ref: string): string[] | null {
  if (!ref.startsWith(OPFS_REF)) return null;
  const parts = ref.slice(OPFS_REF.length).split("/");
  return parts.every((part) => part !== "" && part !== "." && part !== "..") ? parts : null;
}
