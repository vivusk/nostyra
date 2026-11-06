// Scan pass B for one folder (game-library `library_scan_job` on the web): skip the
// unchanged entries, hash + DAT-match + metadata the rest in wasm (`identify`, the
// desktop's identity code), in batches of 50 posted to the page for the upsert.
// Runs in the scan worker.
import type { ScanProgress } from "../../../api/tauri-commands";
import type { ScanProblem } from "../../../api/tauri-events";
import { systemDat } from "../catalog/system-dat";
import type { NostyraWasm } from "../wasm/wasm-module";
import { displayPath, problemOf, type DiscoveredGame, type FolderScan } from "./scan-discovery";
import { registerFolderFiles } from "./scan-file-registry";
import { SCAN_BATCH_SIZE, type ScanMessage, type ScannedMetadata, type ScannedRow } from "./scan-protocol";
import { virtualPathOf } from "./scan-paths";

/** `identify` JSON (crates/web-bindings identity_json.rs `IdentifiedGame`). */
interface IdentifiedGame {
  crc32: string;
  sha1: string;
  title: string;
  datName: string | null;
  region: string | null;
  serial: string | null;
  metadata: ScannedMetadata | null;
}

export async function hashFolder(
  wasm: NostyraWasm,
  scan: FolderScan,
  post: (message: ScanMessage) => void,
): Promise<void> {
  const { token, name } = scan.input;
  registerFolderFiles(wasm, token, scan.files);
  const toHash = scan.games.filter((game) => game.changed);
  const progress: ScanProgress = {
    folderPath: name,
    filesTotal: scan.games.length,
    filesDone: scan.games.length - toHash.length,
    currentPath: null,
  };
  post({ kind: "progress", progress: { ...progress } });
  for (let start = 0; start < toHash.length; start += SCAN_BATCH_SIZE) {
    const batch = toHash.slice(start, start + SCAN_BATCH_SIZE);
    // DATs load asynchronously before the synchronous identify calls.
    const games: ScannedRow[] = [];
    const failures: ScanProblem[] = [];
    let dats = new Map<string, Awaited<ReturnType<typeof systemDat>>>();
    try {
      dats = new Map(await Promise.all(batch.map(async (game) => [game.system.id, await systemDat(game.system)] as const)));
    } catch (error) {
      // DAT download failed: report the batch as failed (not stored, so the next scan retries).
      const message = error instanceof Error ? error.message : String(error);
      for (const game of batch) failures.push(problemOf(name, game.relative, message));
    }
    for (const game of dats.size === 0 ? [] : batch) {
      try {
        const dat = dats.get(game.system.id);
        if (!dat) throw new Error(`no DAT for ${game.system.id}`);
        const identified = JSON.parse(wasm.identify(virtualPathOf(token, game.relative), game.system.id, dat)) as IdentifiedGame;
        games.push(rowOf(game, identified));
      } catch (error) {
        failures.push(problemOf(name, game.relative, error instanceof Error ? error.message : String(error)));
      }
    }
    progress.filesDone += batch.length;
    progress.currentPath = displayPath(name, batch[batch.length - 1].relative);
    post({ kind: "games", token, games, failures, progress: { ...progress } });
  }
}

function rowOf(game: DiscoveredGame, identified: IdentifiedGame): ScannedRow {
  return {
    romPath: game.romPath,
    platformId: game.system.id,
    fileSize: game.size,
    mtime: game.mtime,
    crc32: identified.crc32,
    sha1: identified.sha1,
    title: identified.title,
    region: identified.region,
    datName: identified.datName,
    serial: identified.serial,
    metadata: identified.metadata,
  };
}
