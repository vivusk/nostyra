// node --test: the store-only zip handed to EmulatorJS for multi-track discs — local
// headers, data in place, central directory and end record with matching CRCs.
import assert from "node:assert/strict";
import { test } from "node:test";
import { crc32OfBlob, crc32Update } from "./crc32-stream.ts";
import { storeZip } from "./store-zip.ts";

test("crc32 matches the standard check value, streamed or not", async () => {
  const bytes = new TextEncoder().encode("123456789");
  assert.equal(crc32Update(0, bytes), 0xcbf43926);
  assert.equal(crc32Update(crc32Update(0, bytes.subarray(0, 4)), bytes.subarray(4)), 0xcbf43926);
  assert.equal(await crc32OfBlob(new Blob([bytes])), 0xcbf43926);
});

test("store-only zip layout", async () => {
  const cue = new Blob(['FILE "Track.bin" BINARY\n']);
  const bin = new Blob([new Uint8Array(3000).fill(7)]);
  const parts = [
    { name: "game.cue", data: cue },
    { name: "Track.bin", data: bin },
  ];
  const zip = new DataView(await (await storeZip(parts, crc32OfBlob)).arrayBuffer());
  const text = (at: number, length: number) => new TextDecoder().decode(new Uint8Array(zip.buffer, at, length));
  // End of central directory: 2 entries; then walk the directory.
  const end = zip.byteLength - 22;
  assert.equal(zip.getUint32(end, true), 0x06054b50);
  assert.equal(zip.getUint16(end + 10, true), 2);
  let at = zip.getUint32(end + 16, true);
  const entries: { name: string; crc: number; size: number; offset: number }[] = [];
  for (let i = 0; i < 2; i += 1) {
    assert.equal(zip.getUint32(at, true), 0x02014b50);
    const nameLength = zip.getUint16(at + 28, true);
    entries.push({
      name: text(at + 46, nameLength),
      crc: zip.getUint32(at + 16, true),
      size: zip.getUint32(at + 24, true),
      offset: zip.getUint32(at + 42, true),
    });
    at += 46 + nameLength;
  }
  assert.deepEqual(
    entries.map((e) => [e.name, e.size]),
    [
      ["game.cue", cue.size],
      ["Track.bin", 3000],
    ],
  );
  for (const [index, entry] of entries.entries()) {
    const blob = parts[index].data;
    assert.equal(zip.getUint32(entry.offset, true), 0x04034b50);
    assert.equal(zip.getUint16(entry.offset + 8, true), 0, "stored, not deflated");
    assert.equal(zip.getUint32(entry.offset + 14, true), entry.crc);
    assert.equal(entry.crc, await crc32OfBlob(blob));
    assert.equal(text(entry.offset + 30, entry.name.length), entry.name);
    const dataAt = entry.offset + 30 + zip.getUint16(entry.offset + 26, true);
    assert.deepEqual(new Uint8Array(zip.buffer, dataAt, blob.size), new Uint8Array(await blob.arrayBuffer()));
  }
});
