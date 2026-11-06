// The web scan's worker: pass A (walk + classify + group + mark-missing data) over
// every folder before any hashing, so a file moved between folders finds its old
// row already missing and relinks (desktop `scan_library`); then pass B per folder.
// The page owns the database: everything found is posted to it. Cancel = the page
// terminates this worker (work already posted is kept, like the desktop's
// stop-after-batch).
/// <reference lib="webworker" />
import { allSystems } from "../catalog/web-systems";
import { loadWasm } from "../wasm/wasm-module";
import { discoverFolder, type FolderScan } from "./scan-discovery";
import { hashFolder } from "./scan-hashing";
import type { ScanMessage, ScanRequest } from "./scan-protocol";
import { reasonOf } from "./scan-walk";

const post = (message: ScanMessage) => self.postMessage(message);

async function run(request: ScanRequest): Promise<void> {
  const [wasm, systems] = await Promise.all([loadWasm(), allSystems()]);
  const scans: FolderScan[] = [];
  for (const folder of request.folders) {
    post({ kind: "progress", progress: { folderPath: folder.name, filesTotal: 0, filesDone: 0, currentPath: null } });
    try {
      const { scan, discovered } = await discoverFolder(wasm, folder, systems, request.zipScanning);
      post(discovered);
      scans.push(scan);
    } catch (error) {
      post({ kind: "folder-error", token: folder.token, error: `${folder.name}: ${reasonOf(error)}` });
    }
  }
  for (const scan of scans) {
    try {
      await hashFolder(wasm, scan, post);
    } catch (error) {
      post({ kind: "folder-error", token: scan.input.token, error: `${scan.input.name}: ${reasonOf(error)}` });
    }
  }
  wasm.files_clear();
}

self.onmessage = (event: MessageEvent<ScanRequest>) => {
  if (event.data.kind !== "start") return;
  run(event.data).then(
    () => post({ kind: "done" }),
    (error: unknown) => post({ kind: "error", error: reasonOf(error) }),
  );
};
