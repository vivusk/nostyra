import type { UpdateStatus } from "../../api/tauri-commands-updates";
import {
  useCheckForUpdates,
  useOpenUpdateReleasePage,
  useSetUpdateCheckEnabled,
  useUpdateStatus,
} from "../../hooks/use-update-queries";

const BUTTON =
  "h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";

/** Settings → Updates (desktop): version, startup toggle, Check now + result line. */
export function SettingsUpdatesSection() {
  const { data: status, isPending, isError } = useUpdateStatus();
  const check = useCheckForUpdates();
  const setEnabled = useSetUpdateCheckEnabled();
  const openPage = useOpenUpdateReleasePage();
  // The startup check runs inside the status query (may take up to 90 s offline).
  const busy = isPending || check.isPending;

  return (
    <>
      <p className="m-0 text-sm text-text-soft">
        {status ? `Version ${status.currentVersion}` : isError ? "Version unknown — could not read the update status." : "Version …"}
      </p>
      <label className="flex cursor-pointer items-center gap-3 text-sm text-text-soft">
        <input
          type="checkbox"
          checked={status?.enabled ?? true}
          disabled={isPending || setEnabled.isPending}
          onChange={(e) => setEnabled.mutate(e.target.checked)}
          className="h-4 w-4 accent-[var(--color-accent)]"
        />
        Check for updates at startup
      </label>
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={() => check.mutate()} disabled={busy} className={BUTTON}>
          {check.isPending ? "Checking…" : "Check now"}
        </button>
        <p className="m-0 text-sm text-text-soft" role="status">
          {busy ? "Checking…" : <ResultLine status={status ?? null} onOpen={() => openPage.mutate()} />}
        </p>
      </div>
      {status?.lastCheckedAt != null && !busy && (
        <p className="m-0 text-xs text-muted">Last checked {new Date(status.lastCheckedAt * 1000).toLocaleString()}</p>
      )}
      <p className="m-0 text-xs text-muted">
        Asks GitHub for the latest release at most once a day. Nothing about you or your library is sent.
      </p>
    </>
  );
}

function ResultLine({ status, onOpen }: { status: UpdateStatus | null; onOpen: () => void }) {
  if (!status) return null;
  if (status.checkFailed) return <>Could not reach GitHub — try again later.</>;
  if (status.updateAvailable && status.latest) {
    const skipped = status.latest.version === status.skippedVersion;
    return (
      <>
        Nostyra {status.latest.version} is available{skipped ? " (skipped)" : ""}.{" "}
        <button
          type="button"
          onClick={onOpen}
          className="cursor-pointer border-0 bg-transparent p-0 font-body text-sm font-bold text-link"
        >
          Open release page
        </button>
      </>
    );
  }
  if (status.lastCheckedAt == null) return null; // never checked (e.g. debug build, toggle off)
  return <>You have the latest version.</>;
}
