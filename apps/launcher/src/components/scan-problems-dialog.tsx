import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../api/query-keys";
import type { ScanProblem } from "../api/tauri-events";

/** "Files not added" after a scan (opened from the scan notice's Details): each
 *  file with the reason it was not added (broken cue sheet, unknown disc, bad
 *  header…), copyable as text. Mounted once in the app shell. */
export function ScanProblemsHost() {
  const queryClient = useQueryClient();
  const { data: open = false } = useQuery<boolean>({
    queryKey: queryKeys.scanProblemsOpen,
    queryFn: () => false,
    staleTime: Infinity,
  });
  const { data: problems = [] } = useQuery<ScanProblem[]>({
    queryKey: queryKeys.scanProblems,
    queryFn: () => [],
    staleTime: Infinity,
  });
  if (!open) return null;
  return (
    <ScanProblemsDialog
      problems={problems}
      onClose={() => queryClient.setQueryData(queryKeys.scanProblemsOpen, false)}
    />
  );
}

/** "Last scan: N files not added — Details" (Settings → ROM folders) while the last
 *  scan of this launcher run left problems; nothing otherwise. */
export function LastScanProblemsLink() {
  const queryClient = useQueryClient();
  const { data: problems = [] } = useQuery<ScanProblem[]>({
    queryKey: queryKeys.scanProblems,
    queryFn: () => [],
    staleTime: Infinity,
  });
  if (problems.length === 0) return null;
  return (
    <p className="m-0 text-xs text-warning">
      Last scan: {problems.length} {problems.length === 1 ? "file" : "files"} not added{" "}
      <button
        type="button"
        onClick={() => queryClient.setQueryData(queryKeys.scanProblemsOpen, true)}
        className="cursor-pointer border-0 bg-transparent p-0 font-body text-xs font-bold text-link"
      >
        Details
      </button>
    </p>
  );
}

function ScanProblemsDialog({ problems, onClose }: { problems: ScanProblem[]; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => dialogRef.current?.showModal(), []);
  const text = problems.map((p) => `${p.path}\n  ${p.reason}`).join("\n");

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="scan-problems-title"
      className="m-auto w-[min(720px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="scan-problems-title" className="m-0 font-display text-[22px] font-semibold">
          Files not added
        </h2>
        {problems.length >= 100 && (
          <p className="m-0 text-xs text-muted">Showing the first 100 files; the log lists every one.</p>
        )}
        <ul className="m-0 flex max-h-[50vh] list-none flex-col gap-3 overflow-auto p-0 text-sm">
          {problems.map((p) => (
            <li key={p.path} className="flex flex-col gap-1">
              <span className="break-all text-text">{p.path}</span>
              <span className="text-text-soft">{p.reason}</span>
            </li>
          ))}
        </ul>
        <div className="flex justify-end gap-3">
          <button
            type="button"
            onClick={() => void navigator.clipboard.writeText(text)}
            className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white"
          >
            Copy list
          </button>
          <button
            type="button"
            onClick={onClose}
            className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body font-bold text-white hover:bg-accent"
          >
            Close
          </button>
        </div>
      </div>
    </dialog>
  );
}
