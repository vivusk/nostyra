// The Play path's cloud step (Resume flow + Detail slot load): pull the game's saves
// before `getResumeInfo` / `play_game` when cloud saves are on and signed in. Never
// blocks Play on the cloud: offline → toast and play with this PC's saves; a conflict
// asks once (Keep this PC / Use cloud / Play without syncing) before the resume
// decision. The web edition never calls the command (`capabilities.cloudSaves`).
import { createContext, useCallback, useContext, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { capabilities } from "../backend";
import { cloudCommands, type CloudConflict, type CloudStatus, type ConflictChoice, type SyncGameResult } from "../api/tauri-commands-cloud";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";
import { refreshCloudViews } from "./use-cloud-queries";

/** The dialog answer: a choice, play without syncing, or cancel the Play. */
export type PrePlayAnswer = ConflictChoice | "skip" | "cancel";

/** The Play path conflict dialog waiting for an answer (null = none). */
export interface PrePlayConflict {
  gameId: number;
  conflict: CloudConflict;
  answer: (answer: PrePlayAnswer) => void;
}

/** `true` = go on and launch, `false` = the user cancelled the Play. */
export type SyncBeforePlay = (gameId: number) => Promise<boolean>;

export const CloudPrePlayContext = createContext<SyncBeforePlay>(async () => true);

/** `syncBeforePlay` from `ResumeFlowProvider` (Detail slot load uses it too). */
export function useSyncBeforePlay() {
  return useContext(CloudPrePlayContext);
}

const OFFLINE_NOTICE = "Cloud unreachable — playing with this PC's saves; they upload later";
/** The backend answers within its 15 s budget; this only guards a lost IPC answer. */
const UI_GUARD_MS = 16_000;

/** Resolves to `null` when the backend has not answered within the guard. */
function withGuard<T>(promise: Promise<T>): Promise<T | null> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const guard = new Promise<null>((resolve) => {
    timer = setTimeout(() => resolve(null), UI_GUARD_MS);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

/** State + function behind `CloudPrePlayContext`; the provider renders the dialog. */
export function useCloudPrePlayState() {
  const queryClient = useQueryClient();
  const { showNotice } = useErrorToast();
  const [pending, setPending] = useState<PrePlayConflict | null>(null);

  const report = useCallback(
    (result: SyncGameResult | null) => {
      if (result === null || result.kind === "offline") showNotice(OFFLINE_NOTICE);
      else if (result.kind === "skipped" && !["in use", "signed out", "no content id"].includes(result.reason)) {
        showNotice(`Saves not synced: ${result.reason}`);
      }
    },
    [showNotice],
  );

  /** Runs one pull (or resolve) with the "Syncing saves…" overlay; errors = offline. */
  const step = useCallback(
    async (gameId: number, run: () => Promise<SyncGameResult>) => {
      queryClient.setQueryData(queryKeys.cloudPrePlay, gameId);
      try {
        return await withGuard(run());
      } catch (error) {
        console.warn("cloud pre-play sync failed; playing with this PC's saves", error);
        return null;
      } finally {
        queryClient.setQueryData(queryKeys.cloudPrePlay, null);
      }
    },
    [queryClient],
  );

  const syncBeforePlay = useCallback<SyncBeforePlay>(
    async (gameId) => {
      if (!capabilities.cloudSaves) return true;
      let status: CloudStatus;
      try {
        status = await queryClient.fetchQuery({ queryKey: queryKeys.cloudStatus, queryFn: cloudCommands.status, staleTime: 5_000 });
      } catch {
        return true; // no status: play without the cloud step
      }
      if (!status.signedIn) return true;
      let result = await step(gameId, () => cloudCommands.syncGame(gameId));
      if (result?.kind === "conflict") {
        const conflict = result;
        const answer = await new Promise<PrePlayAnswer>((resolve) => setPending({ gameId, conflict, answer: resolve }));
        setPending(null);
        if (answer === "cancel") return false;
        result = answer === "skip" ? { kind: "synced" } : await step(gameId, () => cloudCommands.resolveConflict(gameId, answer, true));
        if (result?.kind === "conflict") showNotice("Saves still differ — playing with this PC's saves");
      }
      report(result);
      refreshCloudViews(queryClient);
      void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(gameId) });
      return true;
    },
    [queryClient, step, report, showNotice],
  );

  return { syncBeforePlay, pending };
}
