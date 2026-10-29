// RetroAchievements account (Settings), build capabilities and per-game snapshot
// (Detail tab). `achievements-changed` / `ra-logged-out` events invalidate these.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { achievementsCommands } from "../api/tauri-commands-achievements";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

export function useRaAccount() {
  return useQuery({ queryKey: queryKeys.raAccount, queryFn: achievementsCommands.raAccount });
}

/** Static for the app's lifetime. */
export function useAchievementsCapabilities() {
  return useQuery({
    queryKey: queryKeys.achievementsCapabilities,
    queryFn: achievementsCommands.capabilities,
    staleTime: Infinity,
  });
}

export function useGameAchievements(gameId: number) {
  return useQuery({
    queryKey: queryKeys.gameAchievements(gameId),
    queryFn: () => achievementsCommands.gameAchievements(gameId),
  });
}

/** Account or login changed: every view that depends on it refetches. */
export function refreshAccountViews(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.raAccount });
  void queryClient.invalidateQueries({ queryKey: queryKeys.gameAchievementsRoot });
  // Hardcore gating depends on the login.
  void queryClient.invalidateQueries({ queryKey: queryKeys.resumeInfoRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.saveStatesRoot });
}

/** Login errors stay in the form (`error`), not in a toast. */
export function useRaLogin() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ username, password }: { username: string; password: string }) =>
      achievementsCommands.raLogin(username, password),
    onSuccess: (account) => {
      queryClient.setQueryData(queryKeys.raAccount, account);
      refreshAccountViews(queryClient);
    },
  });
}

export function useRaLogout() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: achievementsCommands.raLogout,
    onSettled: () => refreshAccountViews(queryClient),
    onError: (error) => showError("Cannot log out", errorMessage(error)),
  });
}
