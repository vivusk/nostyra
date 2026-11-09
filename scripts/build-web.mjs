// Builds the deployable web edition (nostyra.com/play) end to end:
// wasm module → EmulatorJS (sha256-verified) → libretro-database copy → licence notices
// → `npm run build:web` → bundle check → dist/nostyra-web-<version>/ with
//   play/**              exactly what https://nostyra.com/play/ serves (folder named after
//                        WEB_BASE_PATH in apps/launcher/web-headers.mjs)
//   vercel.headers.json  the `headers` block for the site's vercel.json (web-headers.mjs)
//   DEPLOY.md            handoff guide (docs/web-edition-deploy-guide.md)
//   build-info.json      version, commit, base path, sizes
// plus dist/nostyra-web-<version>.zip, and prints the size of each part.
// Usage: node scripts/build-web.mjs --shell=ps1|sh (called by build-web.ps1 / .sh,
// which pick the platform's helper scripts). Needs wasm-pack 0.15.0, cargo-about 0.9.2,
// `npm ci` in apps/launcher, network for the first EmulatorJS fetch.
import { spawnSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { checkFileLimits, printSizes, sizeParts } from "./build-web-sizes.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const launcher = join(repo, "apps", "launcher");
const shell = process.argv.find((a) => a.startsWith("--shell="))?.slice(8) ?? (process.platform === "win32" ? "ps1" : "sh");
if (!["ps1", "sh"].includes(shell)) throw new Error(`--shell must be ps1 or sh, got ${shell}`);

function run(command, args, options = {}) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, { cwd: repo, stdio: "inherit", ...options });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed (exit ${result.status ?? result.error})`);
}
const helper = (name) =>
  shell === "ps1"
    ? run("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", join(repo, "scripts", `${name}.ps1`)])
    : run("sh", [join(repo, "scripts", `${name}.sh`)]);
const git = (...args) => spawnSync("git", args, { cwd: repo, encoding: "utf8" });

// Version = workspace version; the launcher's package.json must agree (release.ps1 rule).
const version = /\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/.exec(readFileSync(join(repo, "Cargo.toml"), "utf8"))?.[1];
const packageVersion = JSON.parse(readFileSync(join(launcher, "package.json"), "utf8")).version;
if (!version || version !== packageVersion) throw new Error(`Cargo.toml version ${version} ≠ package.json ${packageVersion}`);

// 1–3: wasm module, EmulatorJS, libretro-database (existing helper scripts).
helper("build-web-wasm");
helper("fetch-emulatorjs");
helper("copy-web-libretro-database");

// 4: licence notices served under <base>/notices/ (public/notices/, git-ignored).
const notices = join(launcher, "public", "notices");
rmSync(notices, { recursive: true, force: true });
mkdirSync(notices, { recursive: true });
copyFileSync(join(repo, "LICENSE"), join(notices, "Nostyra-GPL-3.0.txt"));
// Cores whose EmulatorJS archive ships no license.txt (Stella 2014, Handy; M17).
for (const text of ["GPL-2.0.txt", "Handy-zlib.txt"]) {
  copyFileSync(join(repo, "third-party", "core-licences", text), join(notices, text));
}
run("cargo", ["about", "generate", "-m", "crates/web-bindings/Cargo.toml", "--target", "wasm32-unknown-unknown",
  "--locked", "--fail", "-c", "about.toml", "about-web.hbs", "-o", join(notices, "THIRD-PARTY-RUST.txt")]);
run("node", [join(repo, "scripts", "generate-npm-notices.mjs"), join(notices, "THIRD-PARTY-NPM.txt"), "--web"]);

// 5–6: Vite web build, bundle gate.
run("npm run build:web", [], { cwd: launcher, shell: true }); // shell: npm is npm.cmd on Windows
helper("check-web-bundle");

// 7: package.
const name = `nostyra-web-${version}`;
const dist = join(repo, "dist");
const stage = join(dist, name);
const zip = join(dist, `${name}.zip`);
rmSync(stage, { recursive: true, force: true });
rmSync(zip, { force: true });
const { WEB_BASE_PATH, vercelHeaders } = await import(pathToFileURL(join(launcher, "web-headers.mjs")).href);
const served = WEB_BASE_PATH.slice(1); // "play"
cpSync(join(launcher, "dist-web"), join(stage, served), { recursive: true });
writeFileSync(join(stage, "vercel.headers.json"), `${JSON.stringify({ headers: vercelHeaders(WEB_BASE_PATH) }, null, 2)}\n`);
copyFileSync(join(repo, "docs", "web-edition-deploy-guide.md"), join(stage, "DEPLOY.md"));

const parts = sizeParts(join(stage, served));
const commit = git("rev-parse", "--short", "HEAD").stdout.trim();
const dirty = git("status", "--porcelain").stdout.trim() !== "";
// GPL-3.0: the licences page links the source at tag v<version>; it must be this source.
const tagMatches = git("diff", "--quiet", `v${version}`, "--", ".").status === 0 && !dirty;
writeFileSync(
  join(stage, "build-info.json"),
  `${JSON.stringify({ version, commit, dirty, basePath: `${WEB_BASE_PATH}/`, sourceTag: `v${version}`, sourceTagMatches: tagMatches, builtAt: new Date().toISOString(), bytes: parts.bytes }, null, 2)}\n`,
);

if (process.platform === "win32") {
  run(join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe"), ["-a", "-c", "-f", zip, "-C", dist, name]);
} else {
  run("zip", ["-qr", zip, name], { cwd: dist });
}
if (!existsSync(zip)) throw new Error(`${zip} was not written`);

printSizes(parts, zip);
checkFileLimits(parts);
if (!tagMatches) {
  console.warn(
    `\nwarning: this build is not the source of tag v${version} (commit ${commit}${dirty ? ", uncommitted changes" : ""}).` +
      `\n         The licences page links https://github.com/vivusk/nostyra/tree/v${version}: release (tag + push) the` +
      `\n         source of this build before deploying it publicly (GPL-3.0), then rebuild.`,
  );
}
console.log(`\nPackage: ${stage}\nZip:     ${zip}`);
