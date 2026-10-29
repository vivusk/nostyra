// Save-state slots of one game: list and delete. Load = `useLaunchGame()` with
// `{ gameId, loadState: slot }`. Session end invalidates the list (new saves).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { gameplayCommands, type StateSlotId } from "../api/tauri-commands-gameplay";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Used slots: the resume point ("auto") first, then 0–9 (empty when the game has no
 *  ROM to key saves on). */
export function useSaveStates(gameId: number | null) {
  return useQuery({
    queryKey: queryKeys.saveStates(gameId ?? -1),
    queryFn: () => gameplayCommands.listSaveStates(gameId ?? -1),
    enabled: gameId !== null,
  });
}

/** Deletes one slot's (or the resume point's) files; refused ("busy") while that game
 *  is running. Also refreshes the resume info. */
export function useDeleteSaveState() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: ({ gameId, slot }: { gameId: number; slot: StateSlotId }) =>
      gameplayCommands.deleteSaveState(gameId, slot),
    onSettled: (_, __, { gameId }) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(gameId) });
      void queryClient.invalidateQueries({ queryKey: queryKeys.resumeInfo(gameId) });
    },
    onError: (error) => showError("Cannot delete the save", errorMessage(error)),
  });
}
