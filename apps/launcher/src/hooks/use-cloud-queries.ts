// Cloud saves (desktop): status queries, Settings actions and the dialog prompts kept
// in the query cache. `cloud-status-changed` refetches the status + every chip
// (use-backend-event-sync.ts); command answers that return a status are stored directly.
import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { capabilities } from "../backend";
import { errorMessage } from "../api/tauri-commands";
import {
  cloudCommands,
  type CloudConflict,
  type CloudStatus,
  type CloudToggleKey,
  type ConflictChoice,
  type GameCloudStatus,
} from "../api/tauri-commands-cloud";
import type { CloudSyncProgressEvent } from "../api/tauri-events";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Conflict dialog outside the Play path; `conflict` null = details still loading. */
export interface CloudConflictPrompt {
  gameId: number;
  conflict: CloudConflict | null;
}

/** Status (web / capability off: never fetched). */
export function useCloudStatus() {
  return useQuery<CloudStatus>({
    queryKey: queryKeys.cloudStatus,
    queryFn: cloudCommands.status,
    enabled: capabilities.cloudSaves,
    staleTime: 30_000,
  });
}

/** Detail chip state (hidden while signed out). */
export function useCloudGameStatus(gameId: number, enabled: boolean) {
  return useQuery<GameCloudStatus>({
    queryKey: queryKeys.cloudGameStatus(gameId),
    queryFn: () => cloudCommands.gameStatus(gameId),
    enabled: capabilities.cloudSaves && enabled,
  });
}

/** Event-driven local state (progress, prompts): read-only queries with no fetch. */
function useLocalState<T>(queryKey: readonly unknown[]) {
  return useQuery<T | null>({ queryKey, queryFn: () => null, staleTime: Infinity }).data ?? null;
}

export const useCloudPassProgress = () => useLocalState<CloudSyncProgressEvent>(queryKeys.cloudPassProgress);
export const useCloudPrePlay = () => useLocalState<number>(queryKeys.cloudPrePlay);
export const useCloudConflictPrompt = () => useLocalState<CloudConflictPrompt>(queryKeys.cloudConflictPrompt);
export const useCloudBulkPrompt = () => useLocalState<number[]>(queryKeys.cloudBulkPrompt);
export const useCloudClosingWait = () => useLocalState<boolean>(queryKeys.cloudClosingWait);

/** Refetches the status and every Detail chip. */
export function refreshCloudViews(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.cloudStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.cloudGameStatusRoot });
}

/** A Settings action whose answer is the new status. */
function useStatusMutation<A>(run: (arg: A) => Promise<CloudStatus>, failTitle: string) {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.cloudStatus, status);
      void queryClient.invalidateQueries({ queryKey: queryKeys.cloudGameStatusRoot });
    },
    onError: (error) => showError(failTitle, errorMessage(error)),
  });
}

export const useSetCloudLocalFolder = () =>
  useStatusMutation(cloudCommands.setLocalFolder, "Cannot use this folder");
export const useCloudSignOut = () => useStatusMutation(() => cloudCommands.signOut(), "Cannot sign out");

/** Browser sign-in. Errors (`cloud_unavailable` until the provider ships) are shown
 *  inline by the section, not as a toast. */
export function useCloudSignIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: cloudCommands.signIn,
    onSuccess: (status) => queryClient.setQueryData(queryKeys.cloudStatus, status),
  });
}

/** A fire-and-refresh action (Sync now, toggles, "for all"). */
function useRefreshMutation<A>(run: (arg: A) => Promise<void>, failTitle: string) {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: run,
    onSettled: () => refreshCloudViews(queryClient),
    onError: (error) => showError(failTitle, errorMessage(error)),
  });
}

export const useCloudSyncNow = () => useRefreshMutation(() => cloudCommands.syncNow(), "Cannot sync now");
export const useCloudResolveAll = () => useRefreshMutation(cloudCommands.resolveAll, "Cannot resolve the conflicts");
export const useSetCloudToggle = () =>
  useRefreshMutation(
    ({ key, value }: { key: CloudToggleKey; value: boolean }) => cloudCommands.setToggle(key, value),
    "Cannot save the setting",
  );

type Toasts = ReturnType<typeof useErrorToast>;

/** Opens the conflict dialog for a game outside the Play path (chip, a pass that left
 *  one conflict): fetches the details with a pull-only sync (writes nothing on a
 *  conflict), then shows them; a sync that no longer conflicts just refreshes. */
export async function openCloudConflict(queryClient: QueryClient, toasts: Toasts, gameId: number) {
  const setPrompt = (prompt: CloudConflictPrompt | null) => queryClient.setQueryData(queryKeys.cloudConflictPrompt, prompt);
  setPrompt({ gameId, conflict: null });
  // "Decide later" while loading closes the prompt: do not reopen it with the answer.
  const stillLoading = () => {
    const current = queryClient.getQueryData<CloudConflictPrompt | null>(queryKeys.cloudConflictPrompt);
    return current?.gameId === gameId && current.conflict === null;
  };
  try {
    const result = await cloudCommands.syncGame(gameId);
    if (result.kind === "conflict") {
      if (stillLoading()) setPrompt({ gameId, conflict: result });
      return;
    }
    if (stillLoading()) setPrompt(null);
    if (result.kind === "offline") toasts.showNotice("Cloud unreachable — try again later");
    else if (result.kind === "skipped") toasts.showNotice(`Saves not synced: ${result.reason}`);
  } catch (error) {
    if (stillLoading()) setPrompt(null);
    toasts.showError("Cannot read the cloud saves", errorMessage(error));
  } finally {
    refreshCloudViews(queryClient);
    void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(gameId) });
  }
}

export function useOpenCloudConflict() {
  const queryClient = useQueryClient();
  const toasts = useErrorToast();
  return (gameId: number) => openCloudConflict(queryClient, toasts, gameId);
}

/** Resolves one game outside the Play path (full sync: uploads now when kept). */
export function useCloudResolveConflict() {
  const queryClient = useQueryClient();
  const { showError, showNotice } = useErrorToast();
  return useMutation({
    mutationFn: ({ gameId, choice }: { gameId: number; choice: ConflictChoice }) =>
      cloudCommands.resolveConflict(gameId, choice, false),
    onSuccess: (result) => {
      if (result.kind === "offline") showNotice("Cloud unreachable — your choice is applied at the next sync");
      else if (result.kind === "skipped") showNotice(`Saves not synced: ${result.reason}`);
    },
    onError: (error) => showError("Cannot resolve the conflict", errorMessage(error)),
    onSettled: (_, __, { gameId }) => {
      refreshCloudViews(queryClient);
      void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(gameId) });
      void queryClient.invalidateQueries({ queryKey: queryKeys.resumeInfo(gameId) });
    },
  });
}
