// Play online flows. Host: through the Play path (`usePlayGame(gameId, online)`: cloud
// pull + resume choice first, host only), then `netplay_host`. Join / Watch: straight
// to `netplay_join` (guests never pull or resume: they play on the host's save).
// Both seed the lobby dialog and the "Opening room…" / "Joining room…" overlay.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, type GameSessionState } from "../api/query-keys";
import { netplayCommands, type NetplayHostRequest, type NetplayJoinRole } from "../api/tauri-commands-netplay";
import { useErrorToast } from "../components/error-toast";
import { roomErrorText } from "../lib/netplay-rules";
import { promptForMissingPiece } from "./use-game-session";
import { newLobby, useSetNetplayLobby } from "./use-netplay-session";

/** Host options the Play path carries to `netplay_host`. */
export interface NetplayHostOptions {
  players: number;
  allowSpectators: boolean;
}

/** `netplay_host` after the resume decision (used by `ResumeFlowProvider`). */
export function useNetplayHost() {
  const queryClient = useQueryClient();
  const setLobby = useSetNetplayLobby();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: (request: NetplayHostRequest) => netplayCommands.host(request),
    onMutate: ({ gameId }) => {
      setLobby(() => newLobby("host", gameId, null));
      queryClient.setQueryData<GameSessionState>(queryKeys.session, { gameId, phase: "launching", online: "host" });
    },
    onSuccess: ({ code }) => setLobby((lobby) => lobby && { ...lobby, code, opening: false }),
    onError: (error, { gameId, players, allowSpectators }) => {
      setLobby(() => null);
      queryClient.setQueryData(queryKeys.session, null);
      if (promptForMissingPiece(queryClient, error, gameId, { players, allowSpectators })) return;
      showError("Cannot open the room", roomErrorText(error));
    },
  });
}

export interface NetplayJoinRequest {
  code: string;
  role: NetplayJoinRole;
}

/** `netplay_join`; the join dialog shows refusals inline ("Game already started"). */
export function useNetplayJoin() {
  const queryClient = useQueryClient();
  const setLobby = useSetNetplayLobby();
  return useMutation({
    mutationFn: ({ code, role }: NetplayJoinRequest) => netplayCommands.join(code, role),
    onMutate: ({ code, role }) => setLobby(() => newLobby(role, null, code)),
    onSuccess: (gameId) => {
      setLobby((lobby) => lobby && { ...lobby, gameId, opening: false });
      // The runner may already have reported Started: never step back to "launching".
      if (!queryClient.getQueryData<GameSessionState | null>(queryKeys.session)) {
        queryClient.setQueryData<GameSessionState>(queryKeys.session, { gameId, phase: "launching", online: "join" });
      }
    },
    onError: () => setLobby(() => null),
  });
}

/** Lobby Cancel (before the session started): ends the runner. */
export function useQuitSession() {
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: () => netplayCommands.quitSession(),
    onError: (error) => showError("Cannot cancel", roomErrorText(error)),
  });
}
