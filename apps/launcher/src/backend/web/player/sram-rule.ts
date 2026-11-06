// Which SRAM file the web player loads and writes in a game's save folder (plan Q3,
// phase 00 D7). Every MVP system was proven byte-compatible with the desktop core,
// so the web shares `save.srm` (+ `rtc.bin`). Only when the web core's SRAM has
// another length than the existing `save.srm` (an odd cartridge board) does it write
// `save.web.srm` instead, which it then prefers on load; the desktop never reads
// that file. Pure; one test per rule row.

/** Which SRAM file exists in the game folder (and how long `save.srm` is). */
export interface SramFolderFacts {
  webSramExists: boolean;
  /** Length of `save.srm`; null = no such file. */
  sramLength: number | null;
}

export type SramFile = "sram" | "webSram";

/** The file to inject at start: `save.web.srm` first, else `save.srm`, else none. */
export function sramToLoad(facts: SramFolderFacts): SramFile | null {
  if (facts.webSramExists) return "webSram";
  return facts.sramLength === null ? null : "sram";
}

/** The file to write `webLength` bytes to. A session that loaded or wrote
 *  `save.web.srm` keeps writing it; otherwise `save.srm` unless an existing one has
 *  another length (never shrink or grow the desktop's file). */
export function sramToWrite(facts: SramFolderFacts, webLength: number): SramFile {
  if (facts.webSramExists) return "webSram";
  return facts.sramLength === null || facts.sramLength === webLength ? "sram" : "webSram";
}
