// node --test: the D7 SRAM file rule (phase 06 table), one test per row.
import assert from "node:assert/strict";
import { test } from "node:test";
import { sramToLoad, sramToWrite } from "./sram-rule.ts";

test("no file: nothing to load, the first write creates save.srm", () => {
  const facts = { webSramExists: false, sramLength: null };
  assert.equal(sramToLoad(facts), null);
  assert.equal(sramToWrite(facts, 8192), "sram");
});

test("save.srm of the web core's length: loaded and written back (shared with the desktop)", () => {
  const facts = { webSramExists: false, sramLength: 8192 };
  assert.equal(sramToLoad(facts), "sram");
  assert.equal(sramToWrite(facts, 8192), "sram");
});

test("save.srm of another length: loaded, but the web writes save.web.srm", () => {
  const facts = { webSramExists: false, sramLength: 8192 };
  assert.equal(sramToWrite(facts, 2048), "webSram");
});

test("save.web.srm exists: loaded first and kept as the web's file", () => {
  const facts = { webSramExists: true, sramLength: 8192 };
  assert.equal(sramToLoad(facts), "webSram");
  assert.equal(sramToWrite(facts, 8192), "webSram");
  assert.equal(sramToLoad({ webSramExists: true, sramLength: null }), "webSram");
});
