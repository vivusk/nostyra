import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../api/query-keys";
import {
  useCloudBulkPrompt,
  useCloudClosingWait,
  useCloudConflictPrompt,
  useCloudResolveAll,
  useCloudResolveConflict,
  useCloudStatus,
  useOpenCloudConflict,
} from "../hooks/use-cloud-queries";
import { CloudBulkConflictDialog } from "./cloud-bulk-conflict-dialog";
import { CloudConflictDialog } from "./cloud-conflict-dialog";

/** Cloud dialogs outside the Play path, rendered once (desktop only): one game's
 *  conflict (chip / a pass that left one), the bulk dialog after a pass with ≥ 2
 *  conflicts (shown again after a per-game pick while games remain), and the
 *  "Uploading saves…" notice while the window close waits for an upload. */
export function CloudDialogsHost() {
  const queryClient = useQueryClient();
  const prompt = useCloudConflictPrompt();
  const bulk = useCloudBulkPrompt();
  const closing = useCloudClosingWait();
  const { data: status } = useCloudStatus();
  const resolve = useCloudResolveConflict();
  const resolveAll = useCloudResolveAll();
  const openConflict = useOpenCloudConflict();

  const closePrompt = () => queryClient.setQueryData(queryKeys.cloudConflictPrompt, null);
  const closeBulk = () => queryClient.setQueryData(queryKeys.cloudBulkPrompt, null);
  // Games resolved meanwhile (chip, per game) leave the bulk list.
  const remaining = bulk?.filter((id) => status?.conflictGames.includes(id) ?? true) ?? [];

  useEffect(() => {
    if (bulk && status && remaining.length === 0) closeBulk();
  });

  if (closing) return <ClosingWaitNotice />;
  if (prompt) {
    return (
      <CloudConflictDialog
        gameId={prompt.gameId}
        conflict={prompt.conflict}
        skipLabel="Decide later"
        onChoose={(choice) => {
          resolve.mutate({ gameId: prompt.gameId, choice });
          closePrompt();
        }}
        onSkip={closePrompt}
        onClose={closePrompt}
      />
    );
  }
  if (remaining.length > 0) {
    return (
      <CloudBulkConflictDialog
        gameIds={remaining}
        onChooseAll={(choice) => {
          resolveAll.mutate(choice);
          closeBulk();
        }}
        onPickGame={(gameId) => void openConflict(gameId)}
        onClose={closeBulk}
      />
    );
  }
  return null;
}

/** Modal while the backend holds the window close for an upload (≤ 15 s + a running
 *  apply); the backend exits the app itself, so there is nothing to click. */
function ClosingWaitNotice() {
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);
  return (
    <dialog
      ref={dialogRef}
      // Escape must not hide it: the app is about to exit.
      onCancel={(e) => e.preventDefault()}
      aria-labelledby="cloud-closing-title"
      className="m-auto rounded-card border border-border-strong bg-surface-raised p-0 text-text backdrop:bg-black/60"
    >
      <div role="status" className="flex items-center gap-4 px-7 py-5">
        <span aria-hidden="true" className="size-5 animate-spin rounded-full border-2 border-accent border-t-transparent" />
        <span id="cloud-closing-title" className="font-display text-lg font-semibold">
          Uploading saves…
        </span>
      </div>
    </dialog>
  );
}
