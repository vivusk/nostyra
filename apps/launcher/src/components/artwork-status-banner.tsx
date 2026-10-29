import { useArtworkProgress, useCancelArtworkFetch } from "../hooks/use-artwork-queries";

/** "Fetching artwork · 120 / 480" with progress bar and Cancel (same style as the scan banner). */
export function ArtworkStatusBanner() {
  const { data: progress } = useArtworkProgress();
  const cancel = useCancelArtworkFetch();
  if (!progress) return null;
  const percent = progress.total > 0 ? (progress.done / progress.total) * 100 : 0;
  return (
    <div className="flex flex-wrap items-center gap-3.5 rounded-card border border-border bg-surface px-[18px] py-1 text-sm text-text-soft">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#8C7DFF" strokeWidth="2" aria-hidden="true">
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M3 16l5-5 4 4 3-3 6 6" />
      </svg>
      <span role="status">
        Fetching artwork
        {progress.total > 0 && ` · ${progress.done} / ${progress.total}`}
        {progress.currentTitle && <span className="text-muted"> · {progress.currentTitle}</span>}
      </span>
      <div className="h-1.5 min-w-40 flex-1 rounded-sm bg-border-strong">
        <div className="h-1.5 rounded-sm bg-accent transition-[width]" style={{ width: `${percent}%` }} />
      </div>
      <button
        type="button"
        onClick={() => cancel.mutate()}
        className="h-11 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-link"
      >
        Cancel
      </button>
    </div>
  );
}
