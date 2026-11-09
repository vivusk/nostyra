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
import type { WebSystem } from "../catalog/web-systems";
import { SCAN_BATCH_SIZE, type ScanMessage, type ScannedMetadata, type ScannedRow } from "./scan-protocol";
import { romPathOf, virtualPathOf } from "./scan-paths";

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
  const resolved = await resolvePending(wasm, scan);
  const toHash = [...scan.games, ...resolved.games].filter((game) => game.changed);
  const progress: ScanProgress = {
    folderPath: name,
    filesTotal: scan.games.length + resolved.games.length,
    filesDone: scan.games.length + resolved.games.length - toHash.length,
    currentPath: null,
  };
  post({ kind: "progress", progress: { ...progress } });
  if (resolved.failures.length > 0) {
    post({ kind: "games", token, games: [], failures: resolved.failures, progress: { ...progress } });
  }
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

/**
 * Pass B for the DAT-pending files: each candidate's DAT in registry order (wasm
 * `dat_hit`, the DAT handle borrowed, so the cache stays valid); the first hit lists
 * the file as that system, no hit reports the reason + hint, a DAT that cannot load is
 * a problem (retried next scan).
 */
async function resolvePending(
  wasm: NostyraWasm,
  scan: FolderScan,
): Promise<{ games: DiscoveredGame[]; failures: ScanProblem[] }> {
  const { token, name } = scan.input;
  const byId = new Map(scan.systems.map((system) => [system.id, system]));
  const sizes = new Map(scan.files.map((f) => [f.relative, f.file]));
  const games: DiscoveredGame[] = [];
  const failures: ScanProblem[] = [];
  for (const file of scan.pending) {
    const path = virtualPathOf(token, file.relative);
    try {
      let hit: WebSystem | null = null;
      for (const id of file.candidates) {
        const system = byId.get(id);
        if (!system?.webCore) continue;
        if (wasm.dat_hit(path, id, await systemDat(system))) {
          hit = system;
          break;
        }
      }
      const walked = sizes.get(file.relative);
      if (hit && walked) {
        const romPath = romPathOf(token, file.relative);
        games.push({ relative: file.relative, romPath, system: hit, size: walked.size, mtime: walked.lastModified, changed: true });
      } else {
        failures.push(problemOf(name, file.relative, wasm.dat_pending_hint(file.reason, file.candidates)));
      }
    } catch (error) {
      failures.push(problemOf(name, file.relative, error instanceof Error ? error.message : String(error)));
    }
  }
  return { games, failures };
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
