// Pure name rules of the web player's content preparation, mirrored from the
// desktop: the soft-patch chain beside a ROM (`rom_patch::find_patches_for_rom`),
// the files a cue sheet names and the first disc of an m3u playlist (what the
// desktop runner hands PCSX ReARMed). Unit-tested.

/** Patch formats in chain order (`rom_patch::PATCH_EXTENSIONS`). */
const PATCH_EXTENSIONS = ["bps", "ups", "ips"];

/** `Game.sfc` → `Game` (the last extension removed). */
export function stemOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** The patch chain for `romName` among its folder's `siblings`, in apply order:
 *  `Game.bps`, `Game.bps1`, `Game.bps2`… (numbering stops at the first gap), then
 *  the same for ups and ips. Extensions match in any case; the stem exactly
 *  (case-insensitive like Windows paths). */
export function patchChain(romName: string, siblings: readonly string[]): string[] {
  const stem = stemOf(romName).toLowerCase();
  const byExt = new Map<string, string>();
  for (const name of siblings) {
    if (stemOf(name).toLowerCase() === stem) byExt.set(name.slice(name.lastIndexOf(".") + 1).toLowerCase(), name);
  }
  const chain: string[] = [];
  for (const ext of PATCH_EXTENSIONS) {
    const plain = byExt.get(ext);
    if (plain) chain.push(plain);
    for (let number = 1; byExt.has(`${ext}${number}`); number += 1) chain.push(byExt.get(`${ext}${number}`)!);
  }
  return chain;
}

/** Files a cue sheet references (`FILE "Track 01.bin" BINARY`), in order, as written. */
export function cueFiles(cueText: string): string[] {
  const files: string[] = [];
  for (const match of cueText.matchAll(/^\s*FILE\s+(?:"([^"]+)"|(\S+))/gim)) {
    const name = match[1] ?? match[2];
    if (!files.includes(name)) files.push(name);
  }
  return files;
}

/** First entry of an m3u playlist (comments and blank lines skipped). */
export function firstPlaylistEntry(m3uText: string): string | null {
  for (const line of m3uText.split(/\r?\n/)) {
    const entry = line.trim();
    if (entry && !entry.startsWith("#")) return entry;
  }
  return null;
}

/** `dir/sub/x.cue` + `Track.bin` → `dir/sub/Track.bin` (backslashes as on Windows). */
export function siblingPath(of: string, relative: string): string {
  const base = of.slice(0, of.lastIndexOf("/") + 1);
  return base + relative.replaceAll("\\", "/");
}
