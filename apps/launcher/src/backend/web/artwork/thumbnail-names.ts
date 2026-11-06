// No-Intro name → libretro-thumbnails file name candidates: a TypeScript port of
// crates/artwork-cache/src/thumbnail_name_sanitizer.rs (that crate pulls the HTTP
// client and image codecs, so it is not compiled to wasm). Tests use the desktop's
// examples. Rule: each of `& * / : ` < > ? \ | "` becomes `_`; when the exact name is
// missing, trailing `(…)` tags are dropped right to left while the first one (the
// region) stays: `X (USA) (Rev 1)` → `X (USA)`.

const FORBIDDEN_CHARS = new Set(["&", "*", "/", ":", "`", "<", ">", "?", "\\", "|", '"']);

/** `sanitize_thumbnail_name`. */
export function sanitizeThumbnailName(name: string): string {
  return [...name].map((c) => (FORBIDDEN_CHARS.has(c) ? "_" : c)).join("");
}

/** `trailing_tag_starts`: offsets of the `(` opening each trailing tag, rightmost first. */
function trailingTagStarts(name: string): number[] {
  const starts: number[] = [];
  let rest = name.trimEnd();
  while (rest.endsWith(")")) {
    const open = rest.lastIndexOf("(");
    if (open <= 0) break; // the whole name is one group (or none): no title left to keep
    starts.push(open);
    rest = rest.slice(0, open).trimEnd();
  }
  return starts;
}

/** `fallback_names`: shorter names to try after the exact one (unsanitised). */
export function fallbackNames(name: string): string[] {
  const starts = trailingTagStarts(name);
  return starts.slice(0, Math.max(0, starts.length - 1)).map((start) => name.slice(0, start).trimEnd());
}

/** `resolve_names`: every file name to look up, best first, without duplicates. */
export function resolveNames(noIntroName: string): string[] {
  const trimmed = noIntroName.trim();
  const names = [sanitizeThumbnailName(trimmed)];
  for (const fallback of fallbackNames(trimmed)) {
    const sanitized = sanitizeThumbnailName(fallback);
    if (!names.includes(sanitized)) names.push(sanitized);
  }
  return names;
}

/** Bytes kept literal in a URL path segment (`PATH_SEGMENT` of
 *  thumbnail_source_urls.rs): unreserved + `( ) ' , !`; the rest is escaped. */
export function encodePathSegment(segment: string): string {
  return [...new TextEncoder().encode(segment)]
    .map((byte) => {
      const c = String.fromCharCode(byte);
      return /[A-Za-z0-9\-._~()',!]/.test(c) ? c : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`;
    })
    .join("");
}

export type ThumbnailFolder = "Named_Boxarts" | "Named_Snaps" | "Named_Titles";

/** GitHub raw URL of `<folder>/<file_name>.png` (the web's only host: CORS). */
export function thumbnailUrl(repo: string, folder: ThumbnailFolder, fileName: string): string {
  return `https://raw.githubusercontent.com/libretro-thumbnails/${repo}/master/${folder}/${encodePathSegment(fileName)}.png`;
}

/** GitHub tree API URL listing every file of a thumbnail repo. */
export function indexUrl(repo: string): string {
  return `https://api.github.com/repos/libretro-thumbnails/${repo}/git/trees/master?recursive=1`;
}
