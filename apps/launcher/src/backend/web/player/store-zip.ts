// Store-only (uncompressed) zip of disc files for EmulatorJS, which loads a cue sheet
// with its tracks only as one archive. The zip is a `Blob` of headers and the
// original `File` parts, so the browser never copies a 700 MB track into JS memory
// here (phase 00: EmulatorJS still unpacks it into its own heap). Zip32 limits
// (4 GiB per member) are far above any CD track.

export interface ZipPart {
  /** Name inside the archive (no folders). */
  name: string;
  data: Blob;
}

const LOCAL_HEADER = 0x04034b50;
const CENTRAL_HEADER = 0x02014b50;
const END_OF_CENTRAL = 0x06054b50;
const VERSION = 20;
/** General-purpose flag bit 11: names are UTF-8. */
const UTF8_NAMES = 0x0800;

/** The archive of `parts`, in order; `crcOf` = CRC-32 of a part (crc32-stream.ts,
 *  passed in so this module stays import-free for `node --test`). */
export async function storeZip(parts: readonly ZipPart[], crcOf: (data: Blob) => Promise<number>): Promise<Blob> {
  const body: BlobPart[] = [];
  const central: Uint8Array<ArrayBuffer>[] = [];
  let offset = 0;
  for (const part of parts) {
    const name = new TextEncoder().encode(part.name);
    const crc = await crcOf(part.data);
    const size = part.data.size;
    const local = new DataView(new ArrayBuffer(30));
    local.setUint32(0, LOCAL_HEADER, true);
    local.setUint16(4, VERSION, true);
    local.setUint16(6, UTF8_NAMES, true);
    local.setUint32(14, crc, true);
    local.setUint32(18, size, true);
    local.setUint32(22, size, true);
    local.setUint16(26, name.length, true);
    body.push(local.buffer, name, part.data);
    const entry = new DataView(new ArrayBuffer(46));
    entry.setUint32(0, CENTRAL_HEADER, true);
    entry.setUint16(4, VERSION, true);
    entry.setUint16(6, VERSION, true);
    entry.setUint16(8, UTF8_NAMES, true);
    entry.setUint32(16, crc, true);
    entry.setUint32(20, size, true);
    entry.setUint32(24, size, true);
    entry.setUint16(28, name.length, true);
    entry.setUint32(42, offset, true);
    central.push(new Uint8Array(entry.buffer), name);
    offset += 30 + name.length + size;
  }
  const centralSize = central.reduce((sum, bytes) => sum + bytes.length, 0);
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, END_OF_CENTRAL, true);
  end.setUint16(8, parts.length, true);
  end.setUint16(10, parts.length, true);
  end.setUint32(12, centralSize, true);
  end.setUint32(16, offset, true);
  return new Blob([...body, ...central, end.buffer]);
}
