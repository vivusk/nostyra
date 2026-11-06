// `assetUrl` / `resolveAsset` of the web backend: an asset ref (`opfs:artwork/…`,
// or `datadir:saves/…` for a state thumbnail in the Data folder)
// becomes a render-time token `nostyra-asset:<ref>?v=<version>`, and the token an
// object URL of the OPFS file, cached per token (a rewritten cover gets a new
// version, so a new token). "Clear cache" forgets every URL.
import { readDataDirRef } from "../player/data-dir-refs";
import { readOpfsFile } from "./artwork-store";
import { opfsPartsOfRef } from "./artwork-refs";

const ASSET_TOKEN = "nostyra-asset:";

const resolved = new Map<string, Promise<string | null>>();

/** Render-time image URL of an asset ref. */
export function assetToken(ref: string, version?: number | null): string {
  return `${ASSET_TOKEN}${ref}${version ? `?v=${version}` : ""}`;
}

/** Object URL of a token (cached); other URLs pass through; null when the file is gone. */
export function resolveAssetToken(src: string): Promise<string | null> {
  if (!src.startsWith(ASSET_TOKEN)) return Promise.resolve(src);
  let url = resolved.get(src);
  if (!url) {
    url = readToken(src);
    resolved.set(src, url);
    // A missing file may appear later (art fetched after the first render).
    void url.then((value) => value === null && resolved.delete(src));
  }
  return url;
}

async function readToken(src: string): Promise<string | null> {
  const ref = src.slice(ASSET_TOKEN.length).replace(/\?v=\d+$/, "");
  const parts = opfsPartsOfRef(ref);
  const dataFile = await readDataDirRef(ref);
  const file = dataFile !== undefined ? dataFile : parts ? await readOpfsFile(parts) : null;
  return file ? URL.createObjectURL(file) : null;
}

/** Releases every object URL (after the cache was cleared). */
export function forgetResolvedAssets(): void {
  for (const url of resolved.values()) void url.then((value) => value && URL.revokeObjectURL(value));
  resolved.clear();
}
