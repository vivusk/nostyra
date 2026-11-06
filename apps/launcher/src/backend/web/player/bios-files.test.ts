// node --test: BIOS checks of the web (desktop firmware_status.rs / bios_import.rs).
import assert from "node:assert/strict";
import { test } from "node:test";
import { biosChoice, biosFolder, biosImportName, guessBiosRegion, isValidBios } from "./bios-files.ts";

const KIB512 = 512 * 1024;
const firmware = [
  { name: "scph5500.bin", model: "SCPH-5500", md5: "", region: "JP", size: KIB512, subdir: "" },
  { name: "scph5501.bin", model: "SCPH-5501", md5: "", region: "US", size: KIB512, subdir: "" },
];

function image(marker = "  "): Uint8Array {
  const bytes = new Uint8Array(KIB512);
  bytes.set([0x13, 0x00, 0x08, 0x3c, 0x3f]);
  bytes.set(new TextEncoder().encode(marker), 0x7ff51);
  return bytes;
}

test("signature and region marker", () => {
  assert.equal(isValidBios(image()), true);
  assert.equal(isValidBios(new Uint8Array(KIB512)), false);
  const text = new Uint8Array(KIB512);
  text.set(new TextEncoder().encode("PS compatible"), 0x12c);
  assert.equal(isValidBios(text), true);
  assert.equal(guessBiosRegion(image(" E")), "EU");
  assert.equal(guessBiosRegion(image(" J")), "JP");
  assert.equal(guessBiosRegion(image()), "US");
});

test("region choice: own region or any-region file, else any valid file, else HLE", () => {
  const files = [
    { fileName: "scph5501.bin", model: null, region: "US", valid: true },
    { fileName: "broken.bin", model: null, region: "JP", valid: false },
  ];
  assert.equal(biosChoice(files, "US"), "scph5501.bin");
  assert.equal(biosChoice(files, "JP"), "scph5501.bin");
  assert.equal(biosChoice([files[1]], "JP"), null);
  assert.equal(biosChoice([{ fileName: "psp.bin", model: null, region: "ANY", valid: true }], "EU"), "psp.bin");
});

test("import name: size and signature checked, known names canonical, others lower-cased", () => {
  assert.equal(biosImportName("SCPH5501.BIN", image(), firmware, "PlayStation"), "scph5501.bin");
  assert.equal(biosImportName("My BIOS.BIN", image(), firmware, "PlayStation"), "my bios.bin");
  assert.throws(() => biosImportName("x.bin", new Uint8Array(1024), firmware, "PlayStation"), /exactly 512 KiB/);
  assert.throws(() => biosImportName("x.bin", new Uint8Array(KIB512), firmware, "PlayStation"), /signature/);
  assert.equal(biosFolder(firmware), "system");
});
