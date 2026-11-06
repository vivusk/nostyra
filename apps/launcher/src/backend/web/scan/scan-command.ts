// The web library scan, page side (src-tauri scan_commands.rs on the web): one scan
// at a time in a module worker; a request while it runs queues one more pass. The
// page owns the database, so the worker's findings are written here in arrival
// order (missing marks of pass A land before any upsert → moves relink), and the
// desktop's `scan-progress` / `scan-finished` events are emitted with the same
// payloads. Cancel stops the worker; batches already written are kept.
import type { ScanProgress } from "../../../api/tauri-commands";
import type { ScanFinishedEvent } from "../../../api/tauri-events";
import { fileFingerprints, markMissingPaths, upsertScannedBatch } from "../db/game-scan-writes";
import { zipScanning } from "../db/settings-queries";
import { listRomFolders } from "../fs/rom-folders";
import { emit } from "../web-event-bus";
import type { ScanFolderInput, ScanMessage, ScanRequest } from "./scan-protocol";
import { vanishedPaths } from "./scan-paths";

/** First problems kept for the report (`MAX_SCAN_PROBLEMS`). */
const MAX_SCAN_PROBLEMS = 100;

/** `scan-finished` payload plus the web-only count of games the browser cannot play. */
export type WebScanFinished = ScanFinishedEvent & { desktopOnly: number };

let running = false;
let rerun = false;
let progress: ScanProgress | null = null;
let cancelCurrent: (() => void) | null = null;
const finishedHooks: ((cancelled: boolean) => void)[] = [];

export const isScanning = () => running;
export const currentScanProgress = () => progress;

/** Runs after every scan (artwork auto-fetch). */
export function onScanFinished(hook: (cancelled: boolean) => void): void {
  finishedHooks.push(hook);
}

function setProgress(next: ScanProgress): void {
  progress = next;
  emit("scan-progress", next);
}

/** `start_scan`: returns at once; while a scan runs the request is queued. */
export function startScan(): void {
  if (running) {
    rerun = true;
    return;
  }
  running = true;
  setProgress({ folderPath: "", filesTotal: 0, filesDone: 0, currentPath: null });
  void runScans();
}

/** `cancel_scan`: stops the worker after what it already sent. */
export function cancelScan(): void {
  cancelCurrent?.();
}

async function runScans(): Promise<void> {
  const total: WebScanFinished = {
    added: 0, updated: 0, missing: 0, relinked: 0, skipped: 0, failedFiles: 0,
    failures: [], cancelled: false, error: null, desktopOnly: 0,
  };
  const errors: string[] = [];
  do {
    // Per pass (desktop): a rerun re-sees unchanged files, so skipped / failed / folder
    // errors describe the last pass; added / updated / missing / relinked add up.
    rerun = false;
    Object.assign(total, { skipped: 0, failedFiles: 0, failures: [], desktopOnly: 0 });
    errors.length = 0;
    total.cancelled = await scanPass(total, errors).catch((error: unknown) => {
      errors.push(error instanceof Error ? error.message : String(error));
      return false;
    });
  } while (!total.cancelled && rerun);
  rerun = false;
  running = false;
  progress = null;
  total.error = errors.length > 0 ? errors.join("\n") : null;
  emit("scan-finished", total);
  for (const hook of finishedHooks) hook(total.cancelled);
}

/** One pass over the current folders; resolves to "cancelled". */
async function scanPass(total: WebScanFinished, errors: string[]): Promise<boolean> {
  const zip = await zipScanning();
  const inputs: ScanFolderInput[] = [];
  const folderIds = new Map<string, number>();
  for (const folder of await listRomFolders()) {
    if (!folder.stored) {
      errors.push(`${folder.name}: the folder is no longer available; remove it and add it again`);
      continue;
    }
    if ((await folder.stored.handle.queryPermission({ mode: "read" })) !== "granted") {
      errors.push(`${folder.name}: no access this visit; use "Reconnect folders" and scan again`);
      continue;
    }
    folderIds.set(folder.token, folder.id);
    inputs.push({ token: folder.token, name: folder.name, handle: folder.stored.handle, known: await fileFingerprints(folder.id) });
  }
  if (inputs.length === 0) return false;
  const known = new Map(inputs.map((input) => [input.token, input.known]));
  const addFailures = (failures: ScanFinishedEvent["failures"]) => {
    total.failedFiles += failures.length;
    total.failures.push(...failures.slice(0, Math.max(0, MAX_SCAN_PROBLEMS - total.failures.length)));
  };
  const handle = async (message: ScanMessage): Promise<void> => {
    switch (message.kind) {
      case "progress":
        return setProgress(message.progress);
      case "discovered": {
        total.skipped += message.skipped;
        total.desktopOnly += message.desktopOnly;
        addFailures(message.failures);
        const vanished = vanishedPaths(
          Object.keys(known.get(message.token) ?? {}),
          new Set(message.present),
          new Set(message.existing),
          message.unreadable,
          zip,
        );
        total.missing += await markMissingPaths(vanished);
        return;
      }
      case "folder-error":
        errors.push(message.error);
        return;
      case "games": {
        const outcome = await upsertScannedBatch(folderIds.get(message.token) ?? -1, message.games, unixNow());
        total.added += outcome.inserted;
        total.updated += outcome.updated;
        total.relinked += outcome.relinked;
        addFailures(message.failures);
        return setProgress(message.progress);
      }
      case "error":
        errors.push(message.error);
    }
  };
  const worker = new Worker(new URL("./scan-worker.ts", import.meta.url), { type: "module" });
  try {
    return await new Promise<boolean>((resolve) => {
      let chain = Promise.resolve();
      const finish = (cancelled: boolean) => void chain.then(() => resolve(cancelled));
      cancelCurrent = () => {
        worker.terminate();
        finish(true);
      };
      worker.onerror = (event) => {
        errors.push(`scan worker failed: ${event.message}`);
        finish(false);
      };
      worker.onmessage = (event: MessageEvent<ScanMessage>) => {
        const message = event.data;
        chain = chain.then(() => handle(message)).catch((error: unknown) => void errors.push(String(error)));
        if (message.kind === "done" || message.kind === "error") finish(false);
      };
      worker.postMessage({ kind: "start", folders: inputs, zipScanning: zip } satisfies ScanRequest);
    });
  } finally {
    cancelCurrent = null;
    worker.terminate();
  }
}

function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}
