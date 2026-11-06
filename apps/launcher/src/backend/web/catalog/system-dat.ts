// A system's identification data for the wasm module: the bundled No-Intro / Redump
// DAT plus the five metadat attribute files, fetched lazily (first game of the
// system) from `<base>/libretro-database/` (copied there at build time by
// scripts/copy-web-libretro-database.*; HTTP-cached). Missing (404) files give an empty or
// partial index: games then "need review" or lack genre/developer, as on the desktop.
import type { WebDat } from "../wasm-pkg/nostyra_web.js";
import { loadWasm } from "../wasm/wasm-module";
import type { WebSystem } from "./web-systems";

const DATABASE_URL = `${import.meta.env.BASE_URL}libretro-database/`;

const loaded = new Map<string, Promise<WebDat>>();

/** `system`'s DAT + metadat index, loaded once per page / worker. */
export function systemDat(system: WebSystem): Promise<WebDat> {
  let dat = loaded.get(system.id);
  if (!dat) {
    dat = loadSystemDat(system);
    loaded.set(system.id, dat);
    // A failed load (network / 5xx) is not cached: the next scan or lookup retries.
    dat.catch(() => loaded.delete(system.id));
  }
  return dat;
}

async function loadSystemDat(system: WebSystem): Promise<WebDat> {
  const wasm = await loadWasm();
  const folders = wasm.metadata_attribute_folders();
  const [datText, ...metadat] = await Promise.all([
    fetchText(`${DATABASE_URL}${encodeURIComponent(system.datFile)}`),
    ...folders.map((folder) => fetchText(`${DATABASE_URL}metadat/${folder}/${encodeURIComponent(system.metadatFile)}`)),
  ]);
  let dat: WebDat;
  try {
    dat = datText === undefined ? wasm.WebDat.empty() : new wasm.WebDat(system.id, datText);
  } catch (error) {
    console.warn(`${system.datFile} unreadable; games will need review`, error);
    dat = wasm.WebDat.empty();
  }
  const [developer, publisher, genre, releaseyear, maxusers] = metadat;
  dat.set_metadata(system.id, developer, publisher, genre, releaseyear, maxusers);
  return dat;
}

/** Body text; undefined only when the file is missing (404). Network errors and other
 * non-OK statuses throw so a transient failure is never stored as "no DAT". */
async function fetchText(url: string): Promise<string | undefined> {
  try {
    const response = await fetch(url);
    if (response.status === 404) {
      console.warn(`libretro-database file missing: ${url}`);
      return undefined;
    }
    if (!response.ok) throw new Error(`libretro-database ${url}: HTTP ${response.status}`);
    return await response.text();
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("libretro-database ")) throw error;
    throw new Error(`libretro-database ${url} not loaded: ${error instanceof Error ? error.message : String(error)}`);
  }
}
