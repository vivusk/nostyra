// Copies the libretro-database files the web edition fetches at runtime — each
// browser-playable system's DAT plus its five metadat attribute files — from
// third-party/libretro-database/ into apps/launcher/public/libretro-database/
// (git-ignored; Vite ships public/ as is). The system list and file names come from
// the registry through the web wasm module (build it first: scripts/build-web-wasm.*),
// so nothing here repeats system facts. Prints the file count and total size.
import { copyFileSync, mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(repo, "third-party", "libretro-database");
const target = join(repo, "apps", "launcher", "public", "libretro-database");
const pkg = join(repo, "apps", "launcher", "src", "backend", "web", "wasm-pkg");

const wasm = await import(pathToFileURL(join(pkg, "nostyra_web.js")).href);
wasm.initSync({ module: readFileSync(join(pkg, "nostyra_web_bg.wasm")) });
const systems = JSON.parse(wasm.system_catalog_json()).filter((system) => system.webCore !== null);
const folders = wasm.metadata_attribute_folders();

rmSync(target, { recursive: true, force: true });
const files = new Set();
for (const system of systems) {
  files.add(system.datFile);
  for (const folder of folders) files.add(join("metadat", folder, system.metadatFile));
}
let bytes = 0;
for (const file of files) {
  const from = join(source, file);
  const to = join(target, file);
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
  bytes += statSync(to).size;
}
copyFileSync(join(source, "LICENSE"), join(target, "LICENSE"));
copyFileSync(join(source, "SOURCE.txt"), join(target, "SOURCE.txt"));
console.log(`libretro-database for the web: ${systems.length} systems, ${files.size} files, ${(bytes / 1048576).toFixed(1)} MB → ${target}`);
