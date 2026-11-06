// node --test against the real wasm module (scripts/build-web-wasm.* first): a
// game's entry in a Data folder listing, web save-state file names, sidecars and
// the pending play-session record.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as wasm from "../wasm-pkg/nostyra_web.js";
import { entryNameFor, stateFilePaths, stemArgument } from "./web-file-names.ts";

wasm.initSync({ module: readFileSync(new URL("../wasm-pkg/nostyra_web_bg.wasm", import.meta.url)) });
const naming = { find: wasm.find_entry_name, create: wasm.new_entry_name };
const SHA1 = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

test("entry: an existing [ID] folder wins (desktop-made or renamed), else a new name", () => {
  const listing = ["Other [00000000]", "Renamed by hand [6B47BB75]"];
  assert.equal(entryNameFor(listing, SHA1, "Game.sfc", "", naming), "Renamed by hand [6B47BB75]");
  assert.equal(entryNameFor([], SHA1, "Zelda: Link?.sfc", "", naming), "Zelda_ Link_ [6B47BB75]");
  assert.equal(entryNameFor(["Game [6B47BB75].toml"], SHA1, "Game.sfc", ".toml", naming), "Game [6B47BB75].toml");
  assert.equal(entryNameFor([], "not-a-sha1", "Game.sfc", "", naming), null);
});

test("web state files never use the desktop's names", () => {
  assert.equal(wasm.web_state_stem(stemArgument("auto")), "web-auto");
  assert.equal(wasm.web_state_stem(stemArgument(3)), "web-slot3");
  assert.throws(() => stemArgument(10));
  assert.throws(() => stemArgument(1.5));
  assert.deepEqual(stateFilePaths("saves/snes/G [6B47BB75]", "web-slot3"), {
    state: "saves/snes/G [6B47BB75]/web-slot3.state",
    sidecar: "saves/snes/G [6B47BB75]/web-slot3.json",
    thumbnail: "saves/snes/G [6B47BB75]/web-slot3.png",
  });
});

test("web sidecar: desktop format with build web; other cores and builds are refused", () => {
  const json = wasm.web_state_sidecar("snes9x", 1_700_000_000, 0x1234, null, new Uint8Array([1, 2, 3]));
  const sidecar = JSON.parse(json);
  assert.equal(sidecar.build, "web");
  assert.equal(sidecar.core_version, "emulatorjs-4.2.3");
  assert.equal(sidecar.sram_crc32, 0x1234);
  assert.equal(sidecar.state_len, 3);
  assert.equal(sidecar.rom_patch_crc32, undefined);
  assert.equal(wasm.web_state_refusal(json, "snes9x"), undefined);
  assert.equal(wasm.web_state_refusal(json, "mgba"), "Made by another core (snes9x)");
});

test("pending play session record for the desktop import", () => {
  const record = JSON.parse(wasm.web_pending_session_json("web:ab12cd34/Game.sfc", 1_700_000_000, 61, SHA1));
  assert.deepEqual(record, {
    rom_path: "web:ab12cd34/Game.sfc",
    started_at: 1_700_000_000,
    play_seconds: 61,
    exit_code: 0,
    content_sha1: SHA1,
  });
  assert.equal(wasm.web_pending_session_file_name(1_700_000_000_123), "1700000000123-web.json");
});
