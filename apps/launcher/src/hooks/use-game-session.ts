// Core status/download, the session state and the raw launch (Play = use-play-game).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, errorMessage, type CommandError } from "../api/tauri-commands";
import type { StateSlotId } from "../api/tauri-commands-gameplay";
import type { CoreDownloadProgressEvent } from "../api/tauri-events";
import { queryKeys, type GameSessionState } from "../api/query-keys";
import type { CoreInstallPrompt } from "../components/core-install-dialog";
import { useErrorToast } from "../components/error-toast";

/** Running game (null = none); seeded from the backend, then event-driven. */
export function useGameSession() {
  return useQuery<GameSessionState | null>({
    queryKey: queryKeys.session,
    queryFn: async () => {
      const gameId = await commands.getActiveSession();
      return gameId === null ? null : { gameId, phase: "playing" };
    },
    staleTime: Infinity,
  });
}

/** A game id, or a game plus the save state to load at start (Save states tab;
 *  `"auto"` = Continue from the resume point). */
export type PlayRequest = number | { gameId: number; loadState?: StateSlotId };

const playTarget = (request: PlayRequest) => (typeof request === "number" ? { gameId: request } : request);

/** Starts the runner right away (no resume decision): the resume flow
 *  (`usePlayGame`) and the Save states tab (explicit slot / resume point) use it. */
export function useLaunchGame() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: (request: PlayRequest) => {
      const { gameId, loadState } = playTarget(request);
      return commands.playGame(gameId, loadState);
    },
    onMutate: (request) => {
      const { gameId } = playTarget(request);
      queryClient.setQueryData<GameSessionState>(queryKeys.session, { gameId, phase: "launching" });
    },
    onError: (error, request) => {
      queryClient.setQueryData(queryKeys.session, null);
      const { kind, coreId } = (error ?? {}) as Partial<CommandError>;
      if (kind === "core_missing" && coreId) {
        // Offer the core instead of a dead-end error (CoreInstallDialog).
        const prompt: CoreInstallPrompt = { coreId, gameId: playTarget(request).gameId, message: errorMessage(error) };
        queryClient.setQueryData(queryKeys.coreInstallPrompt, prompt);
        return;
      }
      showError("Cannot start the game", errorMessage(error));
    },
  });
}

export function useCoreStatus(coreId: string) {
  // Prefix key: the download-finished event invalidates every core's status.
  return useQuery({ queryKey: [...queryKeys.coreStatus, coreId], queryFn: () => commands.getCoreStatus(coreId) });
}

/** Download progress (null = idle). */
export function useCoreDownloadProgress() {
  return useQuery<CoreDownloadProgressEvent | null>({
    queryKey: queryKeys.coreDownload,
    queryFn: () => null,
    staleTime: Infinity,
  });
}

export function useDownloadCore() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: (coreId: string) => commands.downloadCore(coreId),
    onMutate: (coreId) => {
      queryClient.setQueryData<CoreDownloadProgressEvent>(queryKeys.coreDownload, {
        coreId,
        bytesDone: 0,
        bytesTotal: null,
      });
    },
    onError: (error) => {
      queryClient.setQueryData(queryKeys.coreDownload, null);
      showError("Cannot download the core", errorMessage(error));
    },
  });
}
