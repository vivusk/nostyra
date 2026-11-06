// node --test: pad names ↔ the Gamepad API standard layout, the browser's unused
// binds and the EmulatorJS control map, against the shared catalog and defaults.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import * as wasm from "../backend/web/wasm-pkg/nostyra_web.js";
import { codeFromKeyName } from "./keyboard-event-code.ts";
import {
  EJS_PLAYERS,
  labelStyleOfPadId,
  playerInput,
  standardButtonIndex,
  STANDARD_BUTTON_INPUTS,
  WEB_HOTKEYS,
  webUnusedBinds,
} from "./web-input-map.ts";

wasm.initSync({ module: readFileSync(new URL("../backend/web/wasm-pkg/nostyra_web_bg.wasm", import.meta.url)) });
type Catalog = {
  padActions: string[];
  hotkeys: string[];
  gamepadInputs: { name: string }[];
  defaults: Record<string, Record<string, string[]>>;
};
const catalog = JSON.parse(wasm.input_catalog_json()) as Catalog;
const NON_STANDARD = ["misc1", "left_paddle1", "left_paddle2", "right_paddle1", "right_paddle2", "touchpad"];

test("pad names map to standard buttons both ways, or are flagged non-standard", () => {
  for (const { name } of catalog.gamepadInputs) {
    const index = standardButtonIndex(name);
    if (index === null) assert.ok(NON_STANDARD.includes(name), name);
    else assert.equal(STANDARD_BUTTON_INPUTS[index], name);
  }
  assert.equal(STANDARD_BUTTON_INPUTS.length, catalog.gamepadInputs.length - NON_STANDARD.length);
  assert.equal(standardButtonIndex("left_trigger"), 6);
  assert.equal(standardButtonIndex("guide"), 16);
});

test("every default pad bind is usable in the browser", () => {
  for (const section of ["input_gamepad", "hotkeys_gamepad"]) {
    for (const [action, binds] of Object.entries(catalog.defaults[section])) {
      for (const input of binds) assert.notEqual(standardButtonIndex(input), null, `${section}.${action} ${input}`);
    }
  }
  assert.ok(WEB_HOTKEYS.every((hotkey) => catalog.hotkeys.includes(hotkey)));
});

test("unused binds: inputs without a standard button; stick actions follow the stick", () => {
  assert.deepEqual(webUnusedBinds("input_gamepad.a", ["misc1", "east", "north"]), ["misc1"]);
  assert.deepEqual(webUnusedBinds("input_gamepad.lstick_up", ["dpad_up"]), ["dpad_up"]);
  assert.deepEqual(webUnusedBinds("hotkeys_gamepad.save_state", ["touchpad", "left_stick"]), ["touchpad"]);
  assert.deepEqual(webUnusedBinds("input_keyboard.a", ["KeyX", "KeyC"]), []);
});

test("player input from the default bindings", () => {
  const values: Record<string, unknown> = {};
  for (const [section, actions] of Object.entries(catalog.defaults)) {
    for (const [action, binds] of Object.entries(actions)) values[`${section}.${action}`] = binds;
  }
  values["input_keyboard.select"] = ["SuperRight"];
  values["input_gamepad.a"] = ["east", "north", "misc1"];
  const snes = ["b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r"];
  const input = playerInput(values, snes, codeFromKeyName);
  assert.ok(input.padButtons[0].includes(0), "B = south");
  assert.ok(input.padButtons[1].includes(8) && input.padButtons[3].includes(8), "A on east and north");
  assert.ok(input.padButtons[8].includes(2), "Select = back");
  assert.equal(input.padButtons[6], undefined, "no L2 on a SNES pad");
  assert.deepEqual(input.sticks, { left: false, right: false }, "SNES: left stick = D-pad");
  assert.deepEqual(input.keys.KeyZ, [0]);
  assert.deepEqual(input.keys.MetaRight, [2], "OS key stored as Super*");
  assert.deepEqual(input.hotkeys.keyboard.save_state, ["F1"]);
  assert.deepEqual(input.hotkeys.gamepad.fast_forward_hold, [7]);
  assert.equal(input.hotkeys.keyboard.rewind_hold, undefined, "desktop-only hotkey");
  assert.ok(EJS_PLAYERS === 4);

  const n64 = playerInput(values, ["lstick_up", "rstick_left"], codeFromKeyName);
  assert.deepEqual(n64.sticks, { left: true, right: true });
  assert.deepEqual(n64.keys.KeyI, [19]);
  assert.deepEqual(n64.keys.KeyF, [21]);
  assert.equal(Object.keys(n64.padButtons).length, 0, "stick actions are not buttons");
});

test("label style from the pad id", () => {
  assert.equal(labelStyleOfPadId("DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)"), "sony");
  assert.equal(labelStyleOfPadId("Xbox 360 Controller (XInput STANDARD GAMEPAD)"), "xbox");
});
