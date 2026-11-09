// node --test: friendly names in the cloud conflict dialog.
import assert from "node:assert/strict";
import { test } from "node:test";
import { cloudFileLabel, cloudFileLabels } from "./cloud-file-labels.ts";

test("battery saves", () => {
  assert.equal(cloudFileLabel("save.srm"), "Battery save");
  assert.equal(cloudFileLabel("save.web.srm"), "Battery save");
});

test("state slots and the resume point", () => {
  assert.equal(cloudFileLabel("slot3.state"), "Slot 3");
  assert.equal(cloudFileLabel("slot0.png"), "Slot 0");
  assert.equal(cloudFileLabel("web-slot1.state"), "Slot 1");
  assert.equal(cloudFileLabel("auto.png"), "Resume point");
  assert.equal(cloudFileLabel("auto.state"), "Resume point");
  assert.equal(cloudFileLabel("web-auto.json"), "Resume point");
});

test("memory cards", () => {
  assert.equal(cloudFileLabel("Mcd001.ps2"), "Memory card");
  assert.equal(cloudFileLabel("card1.mcd"), "Memory card");
});

test("folder saves", () => {
  assert.equal(cloudFileLabel("PSP/SAVEDATA/ULUS10041/DATA.BIN"), "Save data ULUS10041");
  assert.equal(cloudFileLabel("PSP/SAVEDATA/ULUS10041/ICON0.PNG"), "Save data ULUS10041");
  assert.equal(cloudFileLabel("GC/USA/Card A/01-GALE-x.gci"), "Memory card");
  assert.equal(cloudFileLabel("Wii/title/00010000/data/banner.bin"), "Save data");
});

test("unknown files keep their name", () => {
  assert.equal(cloudFileLabel("notes.txt"), "notes.txt");
});

test("labels are deduplicated in order", () => {
  assert.deepEqual(cloudFileLabels(["slot3.state", "save.srm", "slot3.png", "slot3.json"]), ["Slot 3", "Battery save"]);
});
