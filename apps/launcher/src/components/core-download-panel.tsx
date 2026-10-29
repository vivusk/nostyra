import type { CoreSetupStatus } from "../api/tauri-commands";
import { useCoreDownloadProgress, useCoreStatus, useDownloadCore, useGameSession } from "../hooks/use-game-session";

/** One core's status + (re-)download with progress (Settings). One download runs at
 *  a time: progress shows only on the core being downloaded. */
export function CoreDownloadPanel({ core }: { core: CoreSetupStatus }) {
  const { data: status } = useCoreStatus(core.id);
  const { data: anyProgress } = useCoreDownloadProgress();
  const { data: session } = useGameSession();
  const download = useDownloadCore();
  const progress = anyProgress?.coreId === core.id ? anyProgress : null;
  const busyElsewhere = Boolean(anyProgress) && !progress;
  const downloading = Boolean(progress) || (Boolean(status?.downloading) && !busyElsewhere);
  const percent = progress?.bytesTotal ? Math.round((progress.bytesDone / progress.bytesTotal) * 100) : null;

  return (
    <div className="flex flex-col gap-3">
      <p className="m-0 text-sm text-text-soft">
        {status?.installed
          ? `${core.displayName} ${status.version ?? "(manually installed)"} is installed.`
          : `${core.displayName} is not installed yet.`}
      </p>
      {downloading ? (
        <div role="status" className="flex items-center gap-3 text-sm text-text-soft">
          <div className="h-1.5 w-60 rounded-sm bg-border-strong">
            <div className="h-1.5 rounded-sm bg-accent transition-[width]" style={{ width: `${percent ?? 5}%` }} />
          </div>
          {percent === null ? "Downloading…" : `Downloading… ${percent}%`}
        </div>
      ) : (
        <button
          type="button"
          onClick={() => download.mutate(core.id)}
          disabled={Boolean(session) || busyElsewhere}
          title={session ? "Close the running game first" : busyElsewhere ? "Another core is downloading" : undefined}
          className="h-11 w-fit cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
        >
          {status?.installed ? "Re-download core" : "Download core"}
        </button>
      )}
    </div>
  );
}
