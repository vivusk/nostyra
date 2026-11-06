// Messages between the launcher page and the player iframe (public/player-boot.js,
// plain JavaScript: keep both sides in step). Files and byte arrays travel by
// structured clone, so a posted `File` stays a `File` in the iframe's realm.
import type { PlayerInput } from "../../../lib/web-input-map";

/** Launcher → player. */
export type ToPlayer =
  | { type: "boot"; boot: PlayerBoot }
  | { type: "flush" }
  | { type: "exit"; autoState: boolean }
  | { type: "load-state"; state: Uint8Array }
  | { type: "message"; text: string }
  | { type: "pause" }
  | { type: "resume" };

/** What the player needs to start a game. */
export interface PlayerBoot {
  /** `EJS_core`. */
  core: string;
  /** The content as EmulatorJS loads it (its name gives the save file stem). */
  rom: File;
  /** `EJS_pathtodata` (ends with `/`). */
  pathToData: string;
  /** Files written below `/data/saves/` before start (`<save dir>/<stem>.srm`). */
  saves: Record<string, Uint8Array>;
  /** BIOS files written beside the content (RetroArch's system dir there). */
  bios: Record<string, Uint8Array>;
  /** State loaded right after start (Continue / a picked slot). */
  loadState: Uint8Array | null;
  /** Shown in the player after start ("Slot 3 is empty"). */
  notice: string | null;
  /** Bindings: `EJS_defaultControls` (pads) + keyboard and hotkeys read by public/player-input.js. */
  input: PlayerInput;
}

/** Player → launcher (`{ from: "player", type, data }`). */
export type FromPlayer =
  | { type: "ready"; data: Record<string, never> }
  | { type: "started"; data: { saveFilePath: string } }
  | { type: "sram"; data: { sram: Uint8Array | null; rtc: Uint8Array | null } }
  | { type: "save-state-request"; data: { state: Uint8Array; screenshot: Uint8Array | null } }
  | { type: "load-state-request"; data: Record<string, never> }
  | { type: "auto-state"; data: { state: Uint8Array; screenshot: Uint8Array | null } }
  /** `fatal`: the game cannot start (EmulatorJS start error, out of memory). */
  | { type: "error"; data: { message: string; fatal: boolean } }
  | { type: "exited"; data: Record<string, never> }
  /** Exit asked from inside the game (Esc / Guide held). */
  | { type: "exit-request"; data: Record<string, never> };

/** Version folder of the vendored EmulatorJS (scripts/fetch-emulatorjs.mjs). */
export const EMULATORJS_VERSION = "4.2.3";

/** Stem every content file is handed to EmulatorJS under, so the SRAM path inside
 *  its file system is known before start (`/data/saves/<dir>/game.srm`) whatever the
 *  ROM is called (EmulatorJS cuts names at `#` / `?`). */
export const CONTENT_STEM = "game";

/** `/data/saves/<save dir>/game.srm` relative to `/data/saves`, and the `.rtc` beside it. */
export function playerSavePaths(saveDir: string): { sram: string; rtc: string } {
  return { sram: `${saveDir}/${CONTENT_STEM}.srm`, rtc: `${saveDir}/${CONTENT_STEM}.rtc` };
}
