// node --test (Node ≥ 23 strips TypeScript types): pure Data-folder helpers.
import assert from "node:assert/strict";
import { test } from "node:test";
import { planCopyWithoutOverwrite } from "./data-folder-copy-plan.ts";
import { isWritableDataPath, looksLikeDesktopDataFolder, splitDataPath } from "./data-paths.ts";

test("data paths are relative and stay inside the folder", () => {
  assert.deepEqual(splitDataPath("saves/snes/Game [6B47BB75]/save.srm"), ["saves", "snes", "Game [6B47BB75]", "save.srm"]);
  for (const bad of ["", "/saves/a", "saves/../nostyra.db", "saves//a", "saves\\a", "./saves"]) {
    assert.throws(() => splitDataPath(bad), bad);
  }
});

test("only saves, config, system and pending sessions are writable", () => {
  assert.equal(isWritableDataPath("sessions/pending/1700000000123-web.json"), true);
  assert.equal(isWritableDataPath("sessions/other.json"), false);
  assert.equal(isWritableDataPath("saves/nes/x/save.srm"), true);
  assert.equal(isWritableDataPath("config/global.toml"), true);
  assert.equal(isWritableDataPath("system/scph5501.bin"), true);
  assert.equal(isWritableDataPath("nostyra.db"), false);
  assert.equal(isWritableDataPath("cores/x.dll"), false);
});

test("desktop data folder detection", () => {
  assert.equal(looksLikeDesktopDataFolder(["nostyra.db", "saves"]), true);
  assert.equal(looksLikeDesktopDataFolder(["config"]), true);
  assert.equal(looksLikeDesktopDataFolder(["Photos", "saves"]), false);
});

test("copy never overwrites and lists conflicts", () => {
  const plan = planCopyWithoutOverwrite(
    ["saves/nes/a/save.srm", "config/global.toml", "saves/snes/b/save.srm"],
    ["config/global.toml", "saves/gb/c/save.srm"],
  );
  assert.deepEqual(plan.copy, ["saves/nes/a/save.srm", "saves/snes/b/save.srm"]);
  assert.deepEqual(plan.conflicts, ["config/global.toml"]);
});
