import { useCreateBugReport, useOpenLogsFolder } from "../../hooks/use-bug-report";

const BUTTON =
  "h-10 shrink-0 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";

/** Settings → About → Logs: open the logs folder, or zip the recent logs for an issue. */
export function SettingsLogsRow() {
  const openFolder = useOpenLogsFolder();
  const bugReport = useCreateBugReport();

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="m-0 flex-1 font-body text-sm font-bold text-text">Logs</h3>
        <button type="button" onClick={() => openFolder.mutate()} className={BUTTON}>
          Open logs folder
        </button>
        <button type="button" onClick={() => bugReport.mutate()} disabled={bugReport.isPending} className={BUTTON}>
          {bugReport.isPending ? "Creating…" : "Create bug report"}
        </button>
      </div>
      <p className="m-0 text-xs text-muted">
        Logs stay on this PC and are cleaned up automatically. A bug report zips the recent logs and system
        details (no games, saves or passwords) so you can attach it to a GitHub issue. Logs contain file paths,
        which may include your user name.
      </p>
    </div>
  );
}
