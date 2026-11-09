// BIOS files on the web (Settings → BIOS, and what the player hands PCSX ReARMed):
// read from the Data folder's `system/` like the desktop (`system_catalog::
// scan_firmware`): files of a BIOS size, known names first in the core's order,
// the core's signature check, region from the name or the image. Without a valid
// file the core boots its HLE BIOS (desktop `bios = auto` parity). The web has no
// md5 check: a model is shown for known file names only.
import type { BiosFile, BiosRegion, CoreBios } from "../../../api/tauri-commands-bios";
import type { FirmwareFile, WebSystem } from "../catalog/web-systems";
import type { DataDir } from "../fs/data-dir";

const ANY_REGION = "ANY";
const REGIONS = ["JP", "US", "EU"];
const SIGNATURE_AT = 0x12c;
const REGION_MARKER_AT = 0x7ff51;

/** The core's signature check (`is_valid_bios`): MIPS `lui t0` at byte 1, or
 *  "PS compatible" at 0x12C. */
export function isValidBios(image: Uint8Array): boolean {
  const lui = [0x00, 0x08, 0x3c, 0x3f].every((byte, i) => image[1 + i] === byte);
  const text = new TextDecoder().decode(image.subarray(SIGNATURE_AT, SIGNATURE_AT + 13));
  return lui || text === "PS compatible";
}

/** Region of an unknown image, as the core reads it (" E" / " J" at 0x7FF51, else US). */
export function guessBiosRegion(image: Uint8Array): string {
  const marker = String.fromCharCode(image[REGION_MARKER_AT] ?? 0, image[REGION_MARKER_AT + 1] ?? 0);
  return marker === " E" ? "EU" : marker === " J" ? "JP" : "US";
}

/** `system/` or `system/<subdir>` of a firmware list. */
export function biosFolder(firmware: readonly FirmwareFile[]): string {
  const subdir = firmware[0]?.subdir ?? "";
  return subdir ? `system/${subdir}` : "system";
}

export interface ScannedBios extends BiosFile {
  file: File;
}

/** BIOS-sized files of the core's folder: known names first (core order), then by name. */
export async function scanBios(data: DataDir, firmware: readonly FirmwareFile[]): Promise<ScannedBios[]> {
  if (firmware.length === 0) return [];
  const folder = biosFolder(firmware);
  const found: (ScannedBios & { rank: number })[] = [];
  for (const entry of await data.list(folder)) {
    const file = entry.kind === "file" ? await data.readFile(`${folder}/${entry.name}`) : null;
    if (!file || !firmware.some((f) => f.size === file.size)) continue;
    const rank = firmware.findIndex((f) => f.name.toLowerCase() === entry.name.toLowerCase());
    const known = rank >= 0 ? firmware[rank] : null;
    const image = new Uint8Array(await file.arrayBuffer());
    found.push({
      file,
      rank: rank >= 0 ? rank : Number.MAX_SAFE_INTEGER,
      fileName: entry.name,
      model: known?.model ?? null,
      region: known?.region ?? guessBiosRegion(image),
      valid: isValidBios(image),
      label: null,
      warning: null,
    });
  }
  found.sort((a, b) => a.rank - b.rank || a.fileName.toLowerCase().localeCompare(b.fileName.toLowerCase()));
  return found.map(({ rank: _rank, ...bios }) => bios);
}

/** What a region boots (`FirmwareReport::choice_for`): a valid file of that region or
 *  an any-region one, else any valid file, else null = HLE. */
export function biosChoice(files: readonly BiosFile[], region: string): string | null {
  const valid = files.filter((f) => f.valid);
  return (valid.find((f) => f.region === region || f.region === ANY_REGION) ?? valid[0])?.fileName ?? null;
}

/** Settings → BIOS rows for one system's core. */
export async function coreBiosView(data: DataDir, system: WebSystem, systemNames: string[], folderLabel: string): Promise<CoreBios> {
  const files = (await scanBios(data, system.firmware)).map(({ file: _file, ...view }) => view);
  const regions = system.firmware.every((f) => f.region === ANY_REGION) ? [ANY_REGION] : REGIONS;
  return {
    coreId: system.coreId,
    coreName: `${system.coreName} (EmulatorJS)`,
    systemNames,
    folder: `${folderLabel} › ${biosFolder(system.firmware)}`,
    files,
    regions: regions.map((region): BiosRegion => ({
      region,
      label: region === ANY_REGION ? "All regions" : region,
      fileName: biosChoice(files, region),
    })),
    hleFallback: true,
    pickExtensions: [],
  };
}

/** Name to store a picked image under (`bios_import::import_name` without md5), or
 *  why it is refused. */
export function biosImportName(pickedName: string, image: Uint8Array, firmware: readonly FirmwareFile[], systemName: string): string {
  const notBios = `${pickedName} is not a ${systemName} BIOS`;
  if (!firmware.some((f) => f.size === image.length)) {
    const largest = Math.max(...firmware.map((f) => f.size));
    throw new Error(`${notBios}: a BIOS file is exactly ${largest >> 10} KiB.`);
  }
  if (!isValidBios(image)) throw new Error(`${notBios} (its signature is missing).`);
  const known = firmware.find((f) => f.name.toLowerCase() === pickedName.toLowerCase());
  return known?.name ?? pickedName.toLowerCase();
}
