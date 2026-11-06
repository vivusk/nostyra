import { test } from "node:test";
import assert from "node:assert/strict";
import {
  extensionOf,
  isSkippedFolder,
  isUnchanged,
  romPathOf,
  splitRomPath,
  splitVirtualPath,
  tokenOfFolderPath,
  vanishedPaths,
  virtualPathOf,
} from "./scan-paths.ts";

test("rom paths and virtual paths round-trip", () => {
  const rom = romPathOf("ab12cd34", "Sub/Game (USA).zip");
  assert.equal(rom, "web:ab12cd34/Sub/Game (USA).zip");
  assert.deepEqual(splitRomPath(rom), { token: "ab12cd34", relative: "Sub/Game (USA).zip" });
  assert.deepEqual(splitVirtualPath(virtualPathOf("ab12cd34", "a/b.x")), { token: "ab12cd34", relative: "a/b.x" });
  assert.equal(tokenOfFolderPath("web:ab12cd34"), "ab12cd34");
  assert.equal(tokenOfFolderPath("E:/ROMs"), null);
  assert.equal(splitRomPath("E:/ROMs/x"), null);
});

test("extensions are lower case and dot-files have none", () => {
  assert.equal(extensionOf("dir.v2/Game.ZIP"), "zip");
  assert.equal(extensionOf(".hidden"), "");
  assert.equal(extensionOf("noext"), "");
});

test("fingerprint needs size and mtime", () => {
  const known = { size: 10, mtime: 5, platformId: "x" };
  assert.ok(isUnchanged(known, 10, 5));
  assert.ok(!isUnchanged(known, 11, 5));
  assert.ok(!isUnchanged(undefined, 10, 5));
});

test("vanished paths follow the desktop rules", () => {
  const known = ["web:t/a.rom", "web:t/b.zip", "web:t/gone.zip", "web:t/locked/c.rom", "web:t/d.rom"];
  const present = new Set(["web:t/d.rom"]);
  const existing = new Set(["web:t/b.zip", "web:t/d.rom"]);
  assert.deepEqual(vanishedPaths(known, present, existing, ["web:t/locked"], false), ["web:t/a.rom", "web:t/gone.zip"]);
  assert.deepEqual(vanishedPaths(known, present, existing, [], true), [
    "web:t/a.rom",
    "web:t/b.zip",
    "web:t/gone.zip",
    "web:t/locked/c.rom",
  ]);
});

test("system folders are skipped", () => {
  assert.ok(isSkippedFolder(".git") && isSkippedFolder("System Volume Information"));
  assert.ok(!isSkippedFolder("Nintendo"));
});
