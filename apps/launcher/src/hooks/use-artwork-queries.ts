// Artwork job: live progress, start/cancel, cache stats/clear, auto-fetch setting.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { artworkCommands } from "../api/tauri-commands-artwork";
import { errorMessage } from "../api/tauri-commands";
import type { ArtworkProgressEvent } from "../api/tauri-events";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Live artwork job progress; null when idle. Events keep it current. */
export function useArtworkProgress() {
  return useQuery<ArtworkProgressEvent | null>({
    queryKey: queryKeys.artworkProgress,
    queryFn: artworkCommands.getArtworkProgress,
    staleTime: Infinity,
  });
}

/** Starts fetching all missing art (no argument) or one game's art. */
export function useStartArtworkFetch() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: (gameId?: number) => artworkCommands.startArtworkFetch(gameId),
    onMutate: () => {
      if (queryClient.getQueryData(queryKeys.artworkProgress)) return;
      queryClient.setQueryData<ArtworkProgressEvent>(queryKeys.artworkProgress, {
        done: 0,
        total: 0,
        currentTitle: null,
      });
    },
    onError: (error) => {
      queryClient.setQueryData(queryKeys.artworkProgress, null);
      showError("Cannot fetch artwork", errorMessage(error));
    },
  });
}

export function useCancelArtworkFetch() {
  return useMutation({ mutationFn: artworkCommands.cancelArtworkFetch });
}

export function useArtworkCacheStats() {
  return useQuery({ queryKey: queryKeys.artworkCacheStats, queryFn: artworkCommands.getArtworkCacheStats });
}

export function useClearArtworkCache() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: artworkCommands.clearArtworkCache,
    onSuccess: () => {
      for (const key of [queryKeys.artworkCacheStats, queryKeys.gamesRoot, queryKeys.gameRoot, queryKeys.home]) {
        void queryClient.invalidateQueries({ queryKey: key });
      }
    },
    onError: (error) => showError("Cannot clear the artwork cache", errorMessage(error)),
  });
}

export function useArtworkAutoFetch() {
  return useQuery({ queryKey: queryKeys.artworkAutoFetch, queryFn: artworkCommands.getArtworkAutoFetch });
}

export function useSetArtworkAutoFetch() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: artworkCommands.setArtworkAutoFetch,
    onSuccess: (_, enabled) => queryClient.setQueryData(queryKeys.artworkAutoFetch, enabled),
    onError: (error) => showError("Cannot save the setting", errorMessage(error)),
  });
}
