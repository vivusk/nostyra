// Cached artwork URLs: backend-provided absolute paths only (never user input),
// served by Tauri's asset protocol (scope = <data>/artwork).
import { convertFileSrc } from "@tauri-apps/api/core";

/** asset:// URL for a cached file; `version` busts the WebView cache when the file is rewritten. */
export function assetUrl(path: string | null, version?: number | null): string | null {
  if (!path) return null;
  const url = convertFileSrc(path);
  return version ? `${url}?v=${version}` : url;
}
