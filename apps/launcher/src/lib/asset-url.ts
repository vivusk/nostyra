// Cached artwork / thumbnail URLs from backend asset refs (desktop: absolute paths
// served by Tauri's asset protocol; web: tokens resolved by `useResolvedAsset`).
import { useEffect, useState } from "react";
import { assetUrl as backendAssetUrl, resolveAsset } from "../backend";

/** URL for a cached file; `version` busts the cache when the file is rewritten. */
export function assetUrl(ref: string | null, version?: number | null): string | null {
  return backendAssetUrl(ref, version);
}

/** A real `<img src>` for an `assetUrl` result: the same string on the desktop (no
 *  extra render), an object URL once the web backend has read the file. */
export function useResolvedAsset(src: string | null): string | null {
  const [resolved, setResolved] = useState<{ src: string | null; url: string | null }>({
    src: null,
    url: null,
  });
  const needsResolve = src !== null && !isPlainUrl(src);
  useEffect(() => {
    if (!needsResolve || src === null) return;
    let cancelled = false;
    void resolveAsset(src).then((url) => {
      if (!cancelled) setResolved({ src, url });
    });
    return () => {
      cancelled = true;
    };
  }, [src, needsResolve]);
  if (src === null) return null;
  if (!needsResolve) return src;
  return resolved.src === src ? resolved.url : null;
}

/** URLs the browser loads as is (desktop asset protocol, http, blob, data). */
function isPlainUrl(src: string): boolean {
  return /^(https?|asset|blob|data|file):/i.test(src);
}
