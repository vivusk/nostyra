// Scan pass A for one folder (game-library `folder_scanner` + `library_scan_discovery`
// on the web): walk, classify each candidate file by content in wasm (unchanged
// cartridges keep their stored system: no read at all), group multi-file disc games
// (wasm `group_disc_files`, desktop rules), and set aside games of systems the
// browser cannot play. Runs in the scan worker.
import type { ScanProblem } from "../../../api/tauri-events";
import type { WebSystem } from "../catalog/web-systems";
import type { NostyraWasm } from "../wasm/wasm-module";
import { registerFolderFiles } from "./scan-file-registry";
import type { FolderDiscovered, ScanFolderInput } from "./scan-protocol";
import { extensionOf, isRomArchive, isUnchanged, romPathOf, virtualPathOf } from "./scan-paths";
import { walkFolder, type WalkedFile } from "./scan-walk";

/** A game entry found by pass A. */
export interface DiscoveredGame {
  relative: string;
  romPath: string;
  system: WebSystem;
  /** Fingerprint: sum of the entry's files, newest `lastModified`. */
  size: number;
  mtime: number;
  changed: boolean;
}

/** What pass B needs of a folder (no second walk). */
export interface FolderScan {
  input: ScanFolderInput;
  files: WalkedFile[];
  games: DiscoveredGame[];
}

type Verdict = { kind: "ok"; system: string } | { kind: "skip" } | { kind: "problem"; reason: string };

interface Classified {
  walked: WalkedFile;
  system: WebSystem;
}

export async function discoverFolder(
  wasm: NostyraWasm,
  input: ScanFolderInput,
  systems: WebSystem[],
  zipScanning: boolean,
): Promise<{ scan: FolderScan; discovered: FolderDiscovered }> {
  const { token, known } = input;
  const walk = await walkFolder(input.handle);
  registerFolderFiles(wasm, token, walk.files);
  const byId = new Map(systems.map((system) => [system.id, system]));
  const extensions = new Set(systems.flatMap((system) => system.extensions));
  const allIds = systems.map((system) => system.id);
  const classified: Classified[] = [];
  const problems = walk.failures.map((failure) => ({ ...failure }));
  let skipped = 0;
  for (const walked of walk.files) {
    const { relative, file } = walked;
    const archived = zipScanning && isRomArchive(relative);
    if (!archived && !extensions.has(extensionOf(relative))) continue;
    const stored = known[romPathOf(token, relative)];
    const storedSystem = stored && byId.get(stored.platformId);
    if (storedSystem?.media === "cartridge" && isUnchanged(stored, file.size, file.lastModified)) {
      classified.push({ walked, system: storedSystem });
      continue;
    }
    const verdict = classifyFile(wasm, virtualPathOf(token, relative), archived, allIds);
    if (verdict.kind === "skip") skipped += 1;
    else if (verdict.kind === "problem") problems.push({ relative, reason: verdict.reason });
    else {
      const system = byId.get(verdict.system);
      if (system) classified.push({ walked, system });
    }
  }
  const grouping = JSON.parse(
    wasm.group_disc_files(
      JSON.stringify(classified.map((c) => ({ path: virtualPathOf(token, c.walked.relative), disc: c.system.media === "disc" }))),
      JSON.stringify(problems.map((p) => virtualPathOf(token, p.relative))),
    ),
  ) as { hiddenFiles: number[]; hiddenFailures: number[]; members: { index: number; paths: string[] }[] };
  const filesByPath = new Map(walk.files.map((f) => [virtualPathOf(token, f.relative), f.file]));
  const hidden = new Set(grouping.hiddenFiles);
  const games: DiscoveredGame[] = [];
  let desktopOnly = 0;
  classified.forEach(({ walked, system }, index) => {
    if (hidden.has(index)) return;
    if (!system.webCore) {
      desktopOnly += 1;
      return;
    }
    let size = walked.file.size;
    let mtime = walked.file.lastModified;
    for (const path of grouping.members.find((m) => m.index === index)?.paths ?? []) {
      const member = filesByPath.get(path);
      if (member) {
        size += member.size;
        mtime = Math.max(mtime, member.lastModified);
      }
    }
    const romPath = romPathOf(token, walked.relative);
    games.push({ relative: walked.relative, romPath, system, size, mtime, changed: !isUnchanged(known[romPath], size, mtime) });
  });
  const hiddenFailures = new Set(grouping.hiddenFailures);
  const failures = problems.filter((_, index) => !hiddenFailures.has(index));
  return {
    scan: { input, files: walk.files, games },
    discovered: {
      kind: "discovered",
      token,
      present: games.map((game) => game.romPath),
      existing: walk.files.map((f) => romPathOf(token, f.relative)),
      unreadable: failures.map((f) => romPathOf(token, f.relative)),
      skipped,
      desktopOnly,
      failures: failures.map((f) => problemOf(input.name, f.relative, f.reason)),
    },
  };
}

function classifyFile(wasm: NostyraWasm, path: string, archived: boolean, allIds: string[]): Verdict {
  try {
    return JSON.parse(archived ? wasm.zip_single_rom(path, allIds) : wasm.classify_for_scan(path)) as Verdict;
  } catch (error) {
    return { kind: "problem", reason: error instanceof Error ? error.message : String(error) };
  }
}

/** A scan-report line: the folder's name + relative path (no full path on the web). */
export function problemOf(folderName: string, relative: string, reason: string): ScanProblem {
  return { path: displayPath(folderName, relative), reason };
}

export function displayPath(folderName: string, relative: string): string {
  return relative ? `${folderName}/${relative}` : folderName;
}
