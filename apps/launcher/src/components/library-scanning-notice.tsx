import { useScanProgress, useSetupState } from "../hooks/use-library-queries";

/** Whether a library scan is running (progress events, or the setup state). */
export function useScanActive(): boolean {
  const { data: progress } = useScanProgress();
  const { data: setup } = useSetupState();
  return Boolean(progress) || Boolean(setup?.scanning);
}

/** Shown instead of "no games" while the ROM folders are being read. */
export function LibraryScanningNotice({ className = "" }: { className?: string }) {
  const { data: progress } = useScanProgress();
  const counted = progress && progress.filesTotal > 0 ? ` · ${progress.filesDone} / ${progress.filesTotal} files` : "";
  return (
    <section
      role="status"
      className={`flex items-center gap-4 rounded-[18px] border border-border bg-surface px-10 py-12 ${className}`}
    >
      <svg className="animate-spin" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#8C7DFF" strokeWidth="2" aria-hidden="true">
        <path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" />
      </svg>
      <div className="flex flex-col gap-1">
        <h1 className="m-0 font-display text-2xl font-bold">Reading your ROM folder…</h1>
        <p className="m-0 text-text-soft">
          {progress?.folderPath ? `${progress.folderPath}${counted}. ` : ""}Games appear here as soon as they are identified.
        </p>
      </div>
    </section>
  );
}
