// `settings` table of the web library DB (same keys and value rules as the desktop's
// storage-db settings: "1"/"0" switches).
import { dbBatch, dbQueryOne } from "./db-client";

export async function getSetting(key: string): Promise<string | null> {
  const row = await dbQueryOne<{ value: string }>("SELECT value FROM settings WHERE key = ?", [key]);
  return row?.value ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  await dbBatch([
    {
      sql: "INSERT INTO settings(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
      params: [key, value],
    },
  ]);
}

/** Desktop setting keys and defaults (src-tauri artwork hook, game-library scan options). */
export const ARTWORK_AUTO_FETCH_SETTING = "artwork_auto_fetch";
export const ZIP_SCAN_SETTING = "scan_zip_archives";
/** Web only: "1" once the first-run Data folder step was answered. */
export const DATA_LOCATION_CHOSEN_SETTING = "web_data_location_chosen";

/** Artwork auto-fetch: on unless set to "0" (desktop rule). */
export async function artworkAutoFetch(): Promise<boolean> {
  return (await getSetting(ARTWORK_AUTO_FETCH_SETTING)) !== "0";
}

/** ZIP scanning: off unless set to "1" (desktop rule). */
export async function zipScanning(): Promise<boolean> {
  return (await getSetting(ZIP_SCAN_SETTING)) === "1";
}
