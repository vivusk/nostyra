// Resume point of one game (mode, availability, warnings) for the Play flow.
// Invalidated when a session ends (new auto.*) and when the resume point is deleted.
import { useQuery } from "@tanstack/react-query";
import { gameplayCommands } from "../api/tauri-commands-gameplay";
import { queryKeys } from "../api/query-keys";

export function useResumeInfo(gameId: number | null) {
  return useQuery({
    queryKey: queryKeys.resumeInfo(gameId ?? -1),
    queryFn: () => gameplayCommands.getResumeInfo(gameId ?? -1),
    enabled: gameId !== null,
  });
}
