// Writes the licence notices of the npm packages that end up in the launcher's bundled
// web UI (apps/launcher/dist): every non-dev package of apps/launcher/package-lock.json,
// plus the build tools that emit some of their own code into the bundle.
// Usage: node scripts/generate-npm-notices.mjs <output file>
// Reads licence files from apps/launcher/node_modules (run `npm ci` first). No dependencies.
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const launcherDir = resolve(dirname(fileURLToPath(import.meta.url)), "..", "apps", "launcher");
// Dev dependencies whose output is part of the bundle: Tailwind's base CSS (preflight)
// and Vite's module-preload helper.
const BUNDLED_BUILD_TOOLS = new Set(["tailwindcss", "vite"]);
const LICENCE_FILE = /^(licen[cs]e|copying)(\.(md|txt))?$|^licen[cs]e-[\w.-]+$/i;

const outFile = process.argv[2];
if (!outFile) {
  console.error("usage: node scripts/generate-npm-notices.mjs <output file>");
  process.exit(2);
}

const lock = JSON.parse(readFileSync(join(launcherDir, "package-lock.json"), "utf8"));
if (!lock.packages) throw new Error("package-lock.json has no 'packages' map (lockfileVersion >= 2 needed)");

const entries = Object.entries(lock.packages)
  .filter(([path]) => path.startsWith("node_modules/"))
  .map(([path, meta]) => ({ path, name: path.slice(path.lastIndexOf("node_modules/") + 13), meta }))
  .filter(({ name, meta }) => !meta.dev || BUNDLED_BUILD_TOOLS.has(name))
  .sort((a, b) => a.name.localeCompare(b.name) || a.path.localeCompare(b.path));

function licenceTexts(packageDir, name) {
  if (!existsSync(packageDir)) return null;
  const files = readdirSync(packageDir).filter((f) => LICENCE_FILE.test(f) && !f.endsWith(".spdx")).sort();
  return files.map((file) => {
    let text = readFileSync(join(packageDir, file), "utf8").replace(/\r\n/g, "\n").trimEnd();
    // Vite's LICENSE.md appends the licences of its own build-time dependencies (not in
    // our bundle): keep only the "Vite core license" part.
    if (name === "vite") text = text.split(/\n# Licenses of bundled dependencies/)[0].trimEnd();
    return { file, text };
  });
}

const rule = "=".repeat(80);
const out = [
  "Nostyra - third-party npm packages",
  "==================================",
  "",
  "The launcher's user interface (inside Nostyra.exe) bundles the npm packages below,",
  "unmodified. Packages marked [build tool] are development tools that emit part of",
  "their own code into the bundle. Source: https://www.npmjs.com/package/<name>",
  "",
  ...entries.map(({ name, meta }) =>
    `${name} ${meta.version} - ${meta.license ?? "licence not declared"}${meta.dev ? " [build tool]" : ""}`),
  "",
];
let missing = 0;
for (const { path, name, meta } of entries) {
  out.push(rule, `${name} ${meta.version}${meta.dev ? " [build tool]" : ""}`,
    `Licence: ${meta.license ?? "not declared in package-lock.json"}`);
  const texts = licenceTexts(join(launcherDir, path), name);
  if (texts === null) throw new Error(`${path} is not installed: run npm ci in apps/launcher`);
  if (texts.length === 0) {
    missing += 1;
    out.push("", `Note: the package contains no licence text file; it is distributed under`,
      `${meta.license ?? "an undeclared licence"} as declared in its package.json. Standard texts:`,
      "https://spdx.org/licenses/", "");
    continue;
  }
  for (const { file, text } of texts) out.push("-".repeat(80) + ` ${file}`, text, "");
}

mkdirSync(dirname(resolve(outFile)), { recursive: true });
writeFileSync(outFile, out.join("\n") + "\n", "utf8");
console.log(`npm notices: ${outFile} (${entries.length} packages, ${missing} without a licence file)`);
