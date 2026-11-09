import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { CloudStatus, CloudToggleKey } from "../../api/tauri-commands-cloud";
import { queryKeys } from "../../api/query-keys";
import { useCloudPassProgress, useCloudSignOut, useCloudSyncNow, useSetCloudToggle } from "../../hooks/use-cloud-queries";
import { relativeTimeInSentence } from "../../lib/relative-time";
import { ConfirmDialog } from "../confirm-dialog";
import { CLOUD_BUTTON, CLOUD_PROVIDER_NAMES } from "./settings-cloud-provider-picker";

const TOGGLES: { key: CloudToggleKey; field: "syncStates" | "syncOnStart"; label: string; help: string }[] = [
  { key: "cloud.sync_states", field: "syncStates", label: "Also sync save states", help: "Slots and the resume point; battery saves always sync" },
  { key: "cloud.sync_on_start", field: "syncOnStart", label: "Sync when Nostyra starts", help: "In the background; games never wait for it" },
];

/** Readable last-result line ("ok", "offline", "conflicts: 2", …). */
function resultText(result: string | null): string | null {
  if (!result || result === "ok") return null;
  if (result === "offline") return "cloud unreachable, will retry";
  if (result === "busy") return "another sync was running";
  return result;
}

/** Signed in: account, last sync + result, Sync now (pass progress), the toggles,
 *  Sign out (confirm: saves stay in the cloud and on this PC). */
export function SettingsCloudAccountRow({ status }: { status: CloudStatus }) {
  const queryClient = useQueryClient();
  const syncNow = useCloudSyncNow();
  const signOut = useCloudSignOut();
  const setToggle = useSetCloudToggle();
  const progress = useCloudPassProgress();
  const [confirmSignOut, setConfirmSignOut] = useState(false);
  const syncing = status.syncing || syncNow.isPending || progress !== null;
  const account =
    status.provider === "local_folder" ? status.localFolder : status.accountLabel ?? CLOUD_PROVIDER_NAMES[status.provider];
  const result = resultText(status.lastResult);
  const conflicts = status.conflictGames;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3 rounded-control bg-surface-raised px-4 py-2 text-sm">
        <span className="flex-1 break-all">
          {CLOUD_PROVIDER_NAMES[status.provider]} · <strong>{account}</strong>
        </span>
        <button
          type="button"
          onClick={() => setConfirmSignOut(true)}
          disabled={signOut.isPending}
          className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
        >
          Sign out
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={() => syncNow.mutate()} disabled={syncing} className={CLOUD_BUTTON}>
          {syncing ? "Syncing…" : "Sync now"}
        </button>
        <p className="m-0 text-sm text-text-soft" role="status">
          {progress && progress.total > 0
            ? `Syncing ${Math.min(progress.done + 1, progress.total)} of ${progress.total}…`
            : status.lastSyncAt !== null
              ? `Last synced ${relativeTimeInSentence(status.lastSyncAt)}${result ? ` — ${result}` : ""}`
              : result ?? "Not synced yet"}
        </p>
      </div>
      {conflicts.length > 0 && (
        <button
          type="button"
          onClick={() => queryClient.setQueryData(queryKeys.cloudBulkPrompt, conflicts)}
          className="w-fit cursor-pointer border-0 bg-transparent p-0 font-body text-sm font-bold text-link"
        >
          {conflicts.length === 1 ? "1 game needs" : `${conflicts.length} games need`} a choice — resolve
        </button>
      )}
      {status.pendingUnits > 0 && <p className="m-0 text-xs text-muted">{status.pendingUnits} saves wait to upload.</p>}
      <div className="flex flex-col gap-2">
        {TOGGLES.map(({ key, field, label, help }) => (
          <label key={key} className="flex cursor-pointer items-start gap-3 text-sm text-text-soft">
            <input
              type="checkbox"
              checked={status[field]}
              disabled={setToggle.isPending}
              onChange={(e) => setToggle.mutate({ key, value: e.target.checked })}
              className="mt-0.5 h-4 w-4 accent-[var(--color-accent)]"
            />
            <span className="flex flex-col">
              {label}
              <span className="text-xs text-muted">{help}</span>
            </span>
          </label>
        ))}
      </div>
      {confirmSignOut && (
        <ConfirmDialog
          title={`Sign out of ${CLOUD_PROVIDER_NAMES[status.provider]}?`}
          confirmLabel="Sign out"
          onConfirm={() => signOut.mutate()}
          onClose={() => setConfirmSignOut(false)}
        >
          Saves stay in your cloud and on this PC. Cloud saves stop syncing until you sign in again.
        </ConfirmDialog>
      )}
    </div>
  );
}
