import { commands } from "../api/tauri-commands";
import { useScanProgress } from "../hooks/use-library-queries";

/** "Scanning <folder> · 186 / 248 files" with progress bar and Cancel (mockup). */
export function ScanStatusBanner() {
  const { data: progress } = useScanProgress();
  if (!progress) return null;
  const percent = progress.filesTotal > 0 ? (progress.filesDone / progress.filesTotal) * 100 : 0;
  return (
    <div className="flex flex-wrap items-center gap-3.5 rounded-card border border-border bg-surface px-[18px] py-1 text-sm text-text-soft">
      <svg className="animate-spin" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#8C7DFF" strokeWidth="2" aria-hidden="true">
        <path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" />
      </svg>
      <span role="status">
        Scanning <b className="text-text">{progress.folderPath || "library"}</b>
        {progress.filesTotal > 0 && ` · ${progress.filesDone} / ${progress.filesTotal} files`}
      </span>
      <div className="h-1.5 min-w-40 flex-1 rounded-sm bg-border-strong">
        <div className="h-1.5 rounded-sm bg-accent transition-[width]" style={{ width: `${percent}%` }} />
      </div>
      <button
        type="button"
        onClick={() => void commands.cancelScan()}
        className="h-11 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-link"
      >
        Cancel
      </button>
    </div>
  );
}
