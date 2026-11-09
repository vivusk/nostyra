// Vendors EmulatorJS 4.2.3 for the web edition into
// apps/launcher/public/emulatorjs/4.2.3/ (git-ignored; Vite ships public/ as is):
// the minified runtime, loader, 7z/zip extractors and, per browser core, the core
// build EmulatorJS picks (`-legacy-wasm.data`, plus `-wasm.data` when the core's
// report asks for WebGL 2) and its report. Runtime files come from the EmulatorJS
// CDN, core builds from the npm packages `@emulatorjs/core-<name>@4.2.3` (phase 00
// D1/D2); every file is checked against the sha256 pinned below. No localization/
// (the player sets EJS_language = en-US) and no -thread builds (no COOP/COEP).
// The core list must match the registry's web cores (read through the wasm module:
// run scripts/build-web-wasm.* first). Files already present and intact are kept.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gunzipSync } from "node:zlib";

const VERSION = "4.2.3";
const CDN = `https://cdn.emulatorjs.org/${VERSION}/data/`;
const NPM = "https://registry.npmjs.org/@emulatorjs";
const LICENSE_URL = `https://raw.githubusercontent.com/EmulatorJS/EmulatorJS/v${VERSION}/LICENSE`;

/** Published path → sha256 (verified 2026-11-05: CDN core files are byte-identical to npm). */
const PINS = {
  "loader.js": "69e0903bf1e2f62ced78895e7e511fa26e11316f7eb734925c35e919ba1287b2",
  "emulator.min.js": "6aec3fd7bb2721255801b0a6af02e47e78b05e28a1822b1f213aacbd348abaee",
  "emulator.min.css": "16406c60b2dc3b04ae9b115e308613e6f567a0cc7068e21d9d0c1e5030fb395e",
  "version.json": "d8991427000a96fb97daefdb62178071e9bbff911760706148e584ae8d69f9e7",
  "compression/extract7z.js": "4ac9933b995a516cb6b3ca4027db860278372a20c21c405d93ceb1998498853a",
  "compression/extractzip.js": "3cc825428724213acd43d30552a78008e0869b2326de17dee240dbff9ee2f629",
  "cores/reports/fceumm.json": "13dbbfba0ea1bea087c97c38f47dd49c3fb8f16c3f5ed5678dd75d352b737132",
  "cores/reports/genesis_plus_gx.json": "5936a8ce8d7f010d5bfdd8c3bba2b2414f103b3a703121e56f2724b24dbe7ff3",
  "cores/reports/mgba.json": "08219f6c855a9d996f04ed21169bb0c5ac64d469a8a536468b9876205b5c268d",
  "cores/reports/pcsx_rearmed.json": "93b99266955ab16e9aa0faa0d6338f4a29b50c197764dd48be945573a5b449c1",
  "cores/reports/snes9x.json": "dc7ac963eb7935a7ac78956235ac0b8912ec785c57026336825aa2ed8031b3ad",
  "cores/reports/stella2014.json": "c9920b95db48f678294daa72a289c19846e74ba69ca6d6094a76fb9f560fb39a",
  "cores/reports/prosystem.json": "e76f6a0fa395b175f6ddde0b574a3cb358dca0c359c6fd37e68b85a549ba21ee",
  "cores/reports/handy.json": "995faa51abe88c8d68c1cbf044f2af63f7a0aca315c7c546d65cb63c57c8e60a",
  "cores/fceumm-legacy-wasm.data": "f1054b094e7149fd6278485bc1b2e51ff75c5259048ddb1134171e53d651f239",
  "cores/genesis_plus_gx-legacy-wasm.data": "2a1edeb68d7ec882149ed3cd5d3aa95a6ac231ebabaf82cdb56175a8b62967cc",
  "cores/mgba-legacy-wasm.data": "e300d2f368c82bf61d77c972c36e367cf16408ec3d50beede080337519a3d696",
  "cores/pcsx_rearmed-legacy-wasm.data": "88faee73d06c597a68a8b78e5621d97feefca134c64b623e65f0a900037456da",
  "cores/pcsx_rearmed-wasm.data": "fe5515f6c29f093f0e8c01824b213804f1f76eb9cb4c97c72fe2cc17606bfbc2",
  "cores/snes9x-legacy-wasm.data": "7d427a575cefad98ff400493fa1d7e892da63fe7bab68979babd9cea0bfaaf3b",
  // M17 (2026-11-08, npm 4.2.3; reports' options are empty: no WebGL 2 build needed).
  "cores/stella2014-legacy-wasm.data": "6dc81bb0c3c5833af69aa750491a6ecd547ec9b91c21bd55ed9a77d5b0a8dd08",
  "cores/prosystem-legacy-wasm.data": "18cb399796b85461bd357bd17b08171b3728758eed170a9992dfc79ee071864e",
  "cores/handy-legacy-wasm.data": "6fadae42b263deca542d5de8ff6b663ebfbf03d2b013b89e980b9273b873bd84",
  LICENSE: "a617fab6d251b3302b1cb2c1c94f52d1c584f6fcb2fc218ec1a475bc08053a61",
};

/** Source repositories whose name differs from the core name (the `repo` in each core's core.json). */
const SOURCE_REPOS = {
  fceumm: "libretro-fceumm",
  genesis_plus_gx: "Genesis-Plus-GX",
  stella2014: "stella2014-libretro",
  prosystem: "prosystem-libretro",
  handy: "libretro-handy",
};

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const target = join(repo, "apps", "launcher", "public", "emulatorjs", VERSION);
const pkg = join(repo, "apps", "launcher", "src", "backend", "web", "wasm-pkg");

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const coreOf = (path) => /^cores\/([a-z0-9_]+)-(?:legacy-)?wasm\.data$/.exec(path)?.[1] ?? null;

async function download(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

/** One member of a .tgz (ustar: 512-byte headers, size in octal at 124). */
function tarMember(tgz, wanted) {
  const tar = gunzipSync(tgz);
  for (let at = 0; at + 512 <= tar.length; ) {
    const name = tar.toString("utf8", at, at + 100).replace(/\0.*$/s, "");
    if (!name) break;
    const size = parseInt(tar.toString("utf8", at + 124, at + 136).replace(/\0.*$/s, "").trim() || "0", 8);
    if (name === wanted) return tar.subarray(at + 512, at + 512 + size);
    at += 512 + Math.ceil(size / 512) * 512;
  }
  throw new Error(`${wanted} not in the package`);
}

const tarballs = new Map();
async function fetchFile(path) {
  const core = coreOf(path);
  if (path === "LICENSE") return download(LICENSE_URL);
  if (!core) return download(CDN + path);
  if (!tarballs.has(core)) tarballs.set(core, await download(`${NPM}/core-${core}/-/core-${core}-${VERSION}.tgz`));
  return tarMember(tarballs.get(core), `package/${path.slice("cores/".length)}`);
}

// Registry check: every web core has its pinned build and report, nothing extra.
const wasm = await import(pathToFileURL(join(pkg, "nostyra_web.js")).href);
wasm.initSync({ module: readFileSync(join(pkg, "nostyra_web_bg.wasm")) });
const registry = new Set(JSON.parse(wasm.system_catalog_json()).flatMap((s) => (s.webCore ? [s.webCore.ejsCore] : [])));
const pinned = new Set(Object.keys(PINS).map(coreOf).filter(Boolean));
for (const core of new Set([...registry, ...pinned])) {
  if (!registry.has(core) || !pinned.has(core) || !PINS[`cores/reports/${core}.json`]) {
    throw new Error(`web core ${core}: registry and pins disagree (update PINS)`);
  }
}

let fetched = 0;
let bytes = 0;
for (const [path, want] of Object.entries(PINS)) {
  const file = join(target, path);
  let data = existsSync(file) ? readFileSync(file) : null;
  if (!data || sha256(data) !== want) {
    data = await fetchFile(path);
    const got = sha256(data);
    if (got !== want) throw new Error(`${path}: sha256 ${got}, expected ${want}`);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, data);
    fetched += 1;
  }
  bytes += data.length;
}
writeFileSync(
  join(target, "SOURCE.txt"),
  [
    `EmulatorJS ${VERSION} (GPL-3.0, LICENSE beside this file): https://github.com/EmulatorJS/EmulatorJS/tree/v${VERSION}`,
    `Runtime files: ${CDN}`,
    ...[...registry].sort().map((core) => `Core ${core}: npm @emulatorjs/core-${core}@${VERSION}, source https://github.com/EmulatorJS/${SOURCE_REPOS[core] ?? core}`),
    "Fetched and sha256-checked by scripts/fetch-emulatorjs.mjs.",
    "",
  ].join("\n"),
);
console.log(`EmulatorJS ${VERSION}: ${Object.keys(PINS).length} files (${fetched} downloaded), ${(bytes / 1048576).toFixed(1)} MB → ${target}`);
