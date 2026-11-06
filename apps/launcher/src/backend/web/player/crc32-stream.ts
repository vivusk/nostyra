// CRC-32 (IEEE, zip's) over a Blob read in chunks, so a 700 MB disc track is never
// held in memory at once. Pure; unit-tested against known values.

const TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

/** Running CRC after `bytes` (start with 0). */
export function crc32Update(crc: number, bytes: Uint8Array): number {
  let c = ~crc >>> 0;
  for (let i = 0; i < bytes.length; i += 1) c = TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return ~c >>> 0;
}

/** CRC-32 of a whole Blob, streamed. */
export async function crc32OfBlob(blob: Blob): Promise<number> {
  let crc = 0;
  const reader = blob.stream().getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return crc;
    crc = crc32Update(crc, value);
  }
}
