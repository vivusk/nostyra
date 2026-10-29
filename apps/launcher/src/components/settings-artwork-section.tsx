import { useArtworkAutoFetch, useArtworkCacheStats, useArtworkProgress, useClearArtworkCache, useSetArtworkAutoFetch, useStartArtworkFetch } from "../hooks/use-artwork-queries";

const BUTTON =
  "h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";

function formatMegabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Settings → Artwork: auto-fetch toggle, fetch missing, cache size, clear cache. */
export function SettingsArtworkSection() {
  const { data: autoFetch = true } = useArtworkAutoFetch();
  const setAutoFetch = useSetArtworkAutoFetch();
  const { data: progress } = useArtworkProgress();
  const { data: stats } = useArtworkCacheStats();
  const startFetch = useStartArtworkFetch();
  const clearCache = useClearArtworkCache();
  const fetching = Boolean(progress);

  return (
    <>
      <label className="flex cursor-pointer items-center gap-3 text-sm text-text-soft">
        <input
          type="checkbox"
          checked={autoFetch}
          onChange={(e) => setAutoFetch.mutate(e.target.checked)}
          className="h-4 w-4 accent-[var(--color-accent)]"
        />
        Download cover art automatically after a scan
      </label>
      <p className="m-0 text-xs text-muted">
        {stats ? `${stats.files} images cached · ${formatMegabytes(stats.bytes)}` : "Cache size unknown"}
        {fetching && progress ? ` · fetching ${progress.done}/${progress.total}` : ""}
      </p>
      <div className="flex flex-wrap gap-3">
        <button type="button" onClick={() => startFetch.mutate(undefined)} disabled={fetching} className={BUTTON}>
          {fetching ? "Fetching artwork…" : "Fetch missing artwork"}
        </button>
        <button
          type="button"
          onClick={() => clearCache.mutate()}
          disabled={fetching || clearCache.isPending}
          title={fetching ? "Wait for the artwork fetch to finish" : undefined}
          className={BUTTON}
        >
          Clear artwork cache
        </button>
      </div>
    </>
  );
}
