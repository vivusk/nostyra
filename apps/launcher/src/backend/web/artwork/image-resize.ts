// Validates downloaded art and produces the cached bytes, like
// crates/artwork-cache artwork_image_processor.rs but with the browser's codecs:
// covers are decoded, fitted within 600×600 and re-encoded as JPEG q0.85; hero
// snaps must be real PNGs and are stored unchanged. Decoding is refused above
// 4 096 px per side.

const COVER_MAX_SIDE = 600;
const MAX_DECODE_SIDE = 4096;
const COVER_JPEG_QUALITY = 0.85;
const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/** Thrown for bytes that are not a usable image (stored as `failed`). */
export class InvalidImage extends Error {}

async function decode(blob: Blob): Promise<ImageBitmap> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(blob);
  } catch (error) {
    throw new InvalidImage(`not a decodable image: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (bitmap.width > MAX_DECODE_SIDE || bitmap.height > MAX_DECODE_SIDE) {
    bitmap.close();
    throw new InvalidImage(`image too large (${bitmap.width}×${bitmap.height})`);
  }
  return bitmap;
}

/** `encode_cover_jpeg`: ≤ 600 px on the longest side, aspect kept, JPEG q0.85. */
export async function encodeCoverJpeg(blob: Blob): Promise<Blob> {
  const bitmap = await decode(blob);
  try {
    const scale = Math.min(1, COVER_MAX_SIDE / Math.max(bitmap.width, bitmap.height));
    const width = Math.max(1, Math.round(bitmap.width * scale));
    const height = Math.max(1, Math.round(bitmap.height * scale));
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d");
    if (!context) throw new InvalidImage("no 2D canvas");
    context.imageSmoothingQuality = "high";
    context.drawImage(bitmap, 0, 0, width, height);
    return await canvas.convertToBlob({ type: "image/jpeg", quality: COVER_JPEG_QUALITY });
  } finally {
    bitmap.close();
  }
}

/** `validate_hero_png`: accepts only a PNG that fully decodes within the size cap. */
export async function validateHeroPng(blob: Blob): Promise<Blob> {
  const head = new Uint8Array(await blob.slice(0, PNG_SIGNATURE.length).arrayBuffer());
  if (!PNG_SIGNATURE.every((byte, index) => head[index] === byte)) throw new InvalidImage("expected PNG");
  (await decode(blob)).close();
  return blob;
}
