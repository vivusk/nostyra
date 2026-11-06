// Size report of the web edition package (scripts/build-web.mjs): bytes per part and the
// Vercel upload limits. Limits from https://vercel.com/docs/limits (read 2026-11-06):
// "Static file uploads: when using the CLI to deploy, the maximum size of the source
// files that can be uploaded is limited to 100 MB for Hobby and 1 GB for Pro" and at most
// 15,000 source files per CLI deployment. The page does not say whether 100 MB is per
// file or per deployment, so both are checked against it (the stricter reading).
import { readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

export const VERCEL_UPLOAD_LIMIT = 100 * 1024 * 1024;
export const VERCEL_FILE_COUNT_LIMIT = 15000;

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(path);
    else yield path;
  }
}

/** Part name of one file, by its path below the served folder. */
function partOf(rel) {
  const core = /^emulatorjs\/[^/]+\/cores\/([a-z0-9_]+)-(?:legacy-)?wasm\.data$/.exec(rel);
  if (core) return `core ${core[1]}`;
  if (rel.startsWith("emulatorjs/")) return "EmulatorJS runtime";
  if (rel.startsWith("libretro-database/")) return "DAT data (libretro-database)";
  if (/^assets\/nostyra_web[^/]*\.wasm$/.test(rel)) return "Nostyra wasm";
  if (/^assets\/sqlite3[^/]*\.wasm$/.test(rel)) return "SQLite wasm";
  if (/^player[^/]*$/.test(rel)) return "player page";
  if (rel.startsWith("notices/") || /^(licences\.html|browser-gate\.js|static-page\.css)$/.test(rel)) return "licences + browser gate";
  return "launcher (JS, CSS, fonts, index.html)";
}

/** Every file of the served folder with its part and size. */
export function sizeParts(served) {
  const files = [...walk(served)].map((path) => {
    const rel = relative(served, path).split(sep).join("/");
    return { rel, part: partOf(rel), size: statSync(path).size };
  });
  const byPart = new Map();
  for (const file of files) byPart.set(file.part, (byPart.get(file.part) ?? 0) + file.size);
  const bytes = Object.fromEntries([...byPart].sort((a, b) => b[1] - a[1]));
  return { files, bytes, total: files.reduce((sum, f) => sum + f.size, 0) };
}

const mb = (n) => `${(n / 1048576).toFixed(2).padStart(7)} MB`;

export function printSizes({ files, bytes, total }, zip) {
  console.log("\nSize per part (served folder):");
  for (const [part, size] of Object.entries(bytes)) console.log(`  ${mb(size)}  ${part}`);
  const largest = files.reduce((a, b) => (b.size > a.size ? b : a));
  console.log(`  ${mb(total)}  total, ${files.length} files; largest ${largest.rel} (${mb(largest.size).trim()})`);
  console.log(`  ${mb(statSync(zip).size)}  zip`);
}

/** Fails when a file (or the whole folder) is over Vercel's 100 MB upload limit. */
export function checkFileLimits({ files, total }) {
  const over = files.filter((f) => f.size >= VERCEL_UPLOAD_LIMIT);
  if (over.length > 0) throw new Error(`files over the Vercel 100 MB limit: ${over.map((f) => f.rel).join(", ")}`);
  if (files.length > VERCEL_FILE_COUNT_LIMIT) throw new Error(`${files.length} files > Vercel's ${VERCEL_FILE_COUNT_LIMIT}`);
  const note = total < VERCEL_UPLOAD_LIMIT ? "under" : "OVER (fine on Pro: 1 GB)";
  console.log(`Vercel limits: every file < 100 MB; whole folder ${note} 100 MB; ${files.length} files ≤ 15,000.`);
}
