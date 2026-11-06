// node --test: DOM `KeyboardEvent.code` ↔ the stored (winit) key names, for every
// key of the shared catalog (crates/config-layers through wasm).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as wasm from "../backend/web/wasm-pkg/nostyra_web.js";
import { codeFromKeyName, isIgnorableKeyEvent, keyNameFromCode } from "./keyboard-event-code.ts";

wasm.initSync({ module: readFileSync(new URL("../backend/web/wasm-pkg/nostyra_web_bg.wasm", import.meta.url)) });
const catalog = JSON.parse(wasm.input_catalog_json()) as { keyboardKeys: string[] };

test("every catalog key round-trips through its DOM code", () => {
  for (const name of catalog.keyboardKeys) {
    const code = codeFromKeyName(name);
    assert.equal(keyNameFromCode(code, catalog.keyboardKeys), name, code);
  }
  assert.equal(codeFromKeyName("SuperLeft"), "MetaLeft");
  assert.equal(codeFromKeyName("KeyZ"), "KeyZ");
});

test("codes outside the catalog cannot be bound", () => {
  assert.equal(keyNameFromCode("MetaRight", catalog.keyboardKeys), "SuperRight");
  assert.equal(keyNameFromCode("Unidentified", catalog.keyboardKeys), null);
  assert.equal(keyNameFromCode("AudioVolumeUp", catalog.keyboardKeys), null);
  assert.equal(isIgnorableKeyEvent({ repeat: true, isComposing: false, code: "KeyA" }), true);
  assert.equal(isIgnorableKeyEvent({ repeat: false, isComposing: false, code: "KeyA" }), false);
});
