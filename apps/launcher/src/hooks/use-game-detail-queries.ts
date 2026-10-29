// Game detail, favorites, metadata edit/reset, genres, reveal in folder.
import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { artworkCommands, type GameDetail, type UserMetadataEdit } from "../api/tauri-commands-artwork";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Detail of one game; `null` data = the id is no longer in the library. */
export function useGameDetail(gameId: number | null) {
  return useQuery({
    queryKey: queryKeys.game(gameId ?? -1),
    queryFn: () => artworkCommands.getGameDetail(gameId ?? -1),
    enabled: gameId !== null,
  });
}

export function useGenres() {
  return useQuery({ queryKey: queryKeys.genres, queryFn: artworkCommands.listGenres });
}

/** Lists that show a game's title, metadata or favorite flag. */
function invalidateGameLists(queryClient: QueryClient) {
  for (const key of [queryKeys.gamesRoot, queryKeys.home, queryKeys.genres]) {
    void queryClient.invalidateQueries({ queryKey: key });
  }
}

/** Optimistic heart toggle; rolls back on error. */
export function useSetFavorite() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: ({ gameId, favorite }: { gameId: number; favorite: boolean }) =>
      artworkCommands.setFavorite(gameId, favorite),
    onMutate: ({ gameId, favorite }) => {
      const key = queryKeys.game(gameId);
      const previous = queryClient.getQueryData<GameDetail | null>(key);
      if (previous) queryClient.setQueryData<GameDetail>(key, { ...previous, favorite });
      return { previous };
    },
    onError: (error, { gameId }, context) => {
      if (context?.previous) queryClient.setQueryData(queryKeys.game(gameId), context.previous);
      showError("Cannot update favorites", errorMessage(error));
    },
    onSettled: (_, __, { gameId }) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.game(gameId) });
      invalidateGameLists(queryClient);
    },
  });
}

export function useUpdateGameMetadata() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ gameId, edit }: { gameId: number; edit: UserMetadataEdit }) =>
      artworkCommands.updateGameMetadata(gameId, edit),
    onSuccess: (detail) => {
      queryClient.setQueryData(queryKeys.game(detail.id), detail);
      invalidateGameLists(queryClient);
    },
  });
}

export function useResetGameMetadata() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: artworkCommands.resetGameMetadata,
    onSuccess: (detail) => {
      queryClient.setQueryData(queryKeys.game(detail.id), detail);
      invalidateGameLists(queryClient);
    },
    onError: (error) => showError("Cannot reset metadata", errorMessage(error)),
  });
}

export function useRevealRomInFolder() {
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: artworkCommands.revealRomInFolder,
    onError: (error) => showError("Cannot open the folder", errorMessage(error)),
  });
}
