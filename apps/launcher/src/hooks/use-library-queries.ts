// Library data: setup state, folders, games, summary, scan progress.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, errorMessage, type GameListQuery, type ScanProgress } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

export function useSetupState() {
  return useQuery({ queryKey: queryKeys.setup, queryFn: commands.getSetupState });
}

export function useLibraryFolders() {
  return useQuery({ queryKey: queryKeys.folders, queryFn: commands.listLibraryFolders });
}

export function useGames(query: GameListQuery) {
  return useQuery({
    queryKey: queryKeys.games(query),
    queryFn: () => commands.listGames(query),
    // Keep the old grid while typing a search instead of flashing empty.
    placeholderData: (previous) => previous,
  });
}

export function useLibrarySummary() {
  return useQuery({ queryKey: queryKeys.summary, queryFn: commands.librarySummary });
}

/** Live scan progress; null when idle. Events keep it current. */
export function useScanProgress() {
  return useQuery<ScanProgress | null>({
    queryKey: queryKeys.scanProgress,
    queryFn: commands.getScanProgress,
    staleTime: Infinity,
  });
}

export function useStartScan() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: commands.startScan,
    onMutate: () => {
      // Show the banner immediately; the first progress event fills in numbers. A scan
      // already running keeps its progress (the backend queues one more pass).
      if (queryClient.getQueryData(queryKeys.scanProgress)) return;
      queryClient.setQueryData<ScanProgress>(queryKeys.scanProgress, {
        folderPath: "",
        filesTotal: 0,
        filesDone: 0,
        currentPath: null,
      });
    },
    onError: (error) => {
      queryClient.setQueryData(queryKeys.scanProgress, null);
      showError("Cannot start the scan", errorMessage(error));
    },
  });
}

/** Adds a folder, then rescans so its games appear. */
export function useAddLibraryFolder() {
  const queryClient = useQueryClient();
  const startScan = useStartScan();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: commands.addLibraryFolder,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.folders });
      void queryClient.invalidateQueries({ queryKey: queryKeys.setup });
      startScan.mutate();
    },
    onError: (error) => showError("Cannot add folder", errorMessage(error)),
  });
}

export function useRemoveLibraryFolder() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: commands.removeLibraryFolder,
    onError: (error) => showError("Cannot remove folder", errorMessage(error)),
    onSuccess: () => {
      for (const key of [queryKeys.folders, queryKeys.gamesRoot, queryKeys.summary, queryKeys.setup]) {
        void queryClient.invalidateQueries({ queryKey: key });
      }
    },
  });
}

/** Removes missing games for good: one (`gameId`) or all (`null`). Refreshes the
 *  lists and Home, and drops the removed games' detail/save/config caches. */
export function useRemoveMissingGames() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: async (gameId: number | null) => {
      if (gameId === null) return commands.removeAllMissingGames();
      await commands.removeMissingGame(gameId);
      return 1;
    },
    onError: (error) => showError("Cannot remove the game", errorMessage(error)),
    onSettled: (_, __, gameId) => {
      for (const key of [queryKeys.gamesRoot, queryKeys.summary, queryKeys.home, queryKeys.genres]) {
        void queryClient.invalidateQueries({ queryKey: key });
      }
      const gameKeys =
        gameId === null
          ? [queryKeys.gameRoot, queryKeys.saveStatesRoot, queryKeys.gameplayConfigRoot, queryKeys.resumeInfoRoot]
          : [queryKeys.game(gameId), queryKeys.saveStates(gameId), queryKeys.gameplayConfig(gameId), queryKeys.resumeInfo(gameId)];
      for (const key of gameKeys) queryClient.removeQueries({ queryKey: key });
    },
  });
}
