import assert from "node:assert/strict";
import { test } from "node:test";
import { headerRules, headersFor, LAUNCHER_CSP, PLAYER_CSP, vercelHeaders } from "../../web-headers.mjs";

const PATHS = [
  "/play",
  "/play/",
  "/play/index.html",
  "/play/player.html",
  "/play/player-boot.js",
  "/play/licences.html",
  "/play/browser-gate.js",
  "/play/assets/index-AbC123.js",
  "/play/assets/nostyra_web_bg-AbC123.wasm",
  "/play/emulatorjs/4.2.3/loader.js",
  "/play/emulatorjs/4.2.3/cores/snes9x-legacy-wasm.data",
  "/play/libretro-database/Nintendo - Game Boy.dat",
  "/play/libretro-database/metadat/genre/Nintendo - Game Boy.dat",
];

const pattern = (source: string) => new RegExp(`^${source.replace(/:[A-Za-z]\w*(?=\()/g, "")}$`);

test("no two rules set the same header on one path (host merge order never matters)", () => {
  for (const path of PATHS) {
    const seen = new Map<string, string>();
    for (const { source, headers } of headerRules()) {
      if (!pattern(source).test(path)) continue;
      for (const key of Object.keys(headers)) {
        assert.equal(seen.get(key), undefined, `${path}: ${key} from ${seen.get(key)} and ${source}`);
        seen.set(key, source);
      }
    }
  }
});

test("every path gets CSP, cache policy, nosniff and referrer policy", () => {
  for (const path of PATHS) {
    const headers = headersFor(path);
    for (const key of ["Content-Security-Policy", "Cache-Control", "X-Content-Type-Options", "Referrer-Policy"]) {
      assert.ok(headers[key], `${path}: ${key}`);
    }
  }
});

test("only the player page may eval (EmulatorJS extractors, spike D3)", () => {
  assert.equal(headersFor("/play/player.html")["Content-Security-Policy"], PLAYER_CSP);
  assert.match(PLAYER_CSP, /script-src [^;]*'unsafe-eval'/);
  assert.doesNotMatch(LAUNCHER_CSP, /'unsafe-eval'/);
  for (const path of ["/play", "/play/", "/play/index.html", "/play/player-boot.js"]) {
    assert.equal(headersFor(path)["Content-Security-Policy"], LAUNCHER_CSP, path);
  }
  assert.match(LAUNCHER_CSP, /connect-src [^;]*https:\/\/raw\.githubusercontent\.com[^;]*https:\/\/api\.github\.com/);
});

test("cache: pages revalidate, hashed and versioned files are immutable, DATs one day", () => {
  assert.equal(headersFor("/play/")["Cache-Control"], "no-cache");
  assert.equal(headersFor("/play/player.html")["Cache-Control"], "no-cache");
  assert.match(headersFor("/play/assets/index-AbC123.js")["Cache-Control"], /immutable/);
  assert.match(headersFor("/play/emulatorjs/4.2.3/loader.js")["Cache-Control"], /immutable/);
  assert.match(headersFor("/play/libretro-database/x.dat")["Cache-Control"], /max-age=86400/);
});

test("content types of wasm and core data", () => {
  assert.equal(headersFor("/play/assets/sqlite3-x.wasm")["Content-Type"], "application/wasm");
  assert.equal(headersFor("/play/emulatorjs/4.2.3/cores/mgba-legacy-wasm.data")["Content-Type"], "application/octet-stream");
  assert.equal(headersFor("/play/index.html")["Content-Type"], undefined);
});

test("vercel block is key/value pairs under /play", () => {
  for (const rule of vercelHeaders()) {
    assert.ok(rule.source === "/play" || rule.source.startsWith("/play/"));
    for (const header of rule.headers) assert.deepEqual(Object.keys(header), ["key", "value"]);
  }
});
