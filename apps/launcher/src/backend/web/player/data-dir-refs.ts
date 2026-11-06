// Asset refs of files in the Data folder (state thumbnails `web-slot3.png`): what
// `SaveStateSlot.thumbnailPath` carries on the web instead of an absolute path.
// `assetUrl` turns them into tokens and `resolveAsset` into object URLs, like the
// `opfs:` artwork refs (artwork/asset-resolver.ts).
import { currentDataDir } from "../fs/data-folder";

const DATA_DIR_REF = "datadir:";

/** Ref of a Data folder file (relative path, e.g. `saves/snes/Game [ID]/web-auto.png`). */
export function dataDirRef(path: string): string {
  return `${DATA_DIR_REF}${path}`;
}

/** The file a `datadir:` ref names; undefined when `ref` is not one (null = gone). */
export async function readDataDirRef(ref: string): Promise<File | null | undefined> {
  if (!ref.startsWith(DATA_DIR_REF)) return undefined;
  return (await currentDataDir()).readFile(ref.slice(DATA_DIR_REF.length)).catch(() => null);
}
