// The online room the lobby dialog shows: seeded by the host / join flows
// (`use-netplay-play`), then folded from the runner's netplay events and the session
// end (`use-backend-event-sync`). Lives in the query cache like the game session.
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { listen, type Unlisten } from "../backend";
import { queryKeys } from "../api/query-keys";
import { EVENTS, type NetplayEventPayload, type NetplayRole, type NetplayRosterEntry } from "../api/tauri-events";
import { nextLobbyPhase, type LobbyPhase } from "../lib/netplay-rules";

export interface NetplayLobby {
  role: NetplayRole;
  /** The local game (join: known once the backend found it). */
  gameId: number | null;
  /** Room code (host: from the server; guests: what they typed). */
  code: string | null;
  /** REST call + runner start still running ("Opening room…" overlay, no dialog yet). */
  opening: boolean;
  phase: LobbyPhase;
  roster: NetplayRosterEntry[];
  /** The core is offered as Experimental (PlayStation): warning line. */
  experimental: boolean;
  /** Last in-game stats (ping to the host, relayed link). */
  stats: { pingMs: number | null; relayed: boolean } | null;
  /** Peer left / desync, shown under the status. */
  notice: string | null;
}

/** A fresh room entry for `role` (before any runner event). */
export function newLobby(role: NetplayRole, gameId: number | null, code: string | null): NetplayLobby {
  return {
    role,
    gameId,
    code,
    opening: true,
    phase: { kind: "connecting" },
    roster: [],
    experimental: false,
    stats: null,
    notice: null,
  };
}

/** The lobby after one runner event of the running game. */
export function foldNetplayEvent(lobby: NetplayLobby | null, payload: NetplayEventPayload): NetplayLobby | null {
  if (!lobby) return lobby;
  const next: NetplayLobby = { ...lobby, phase: nextLobbyPhase(lobby.phase, payload) };
  switch (payload.event) {
    case "Lobby":
      return { ...next, roster: payload.roster, experimental: payload.experimental };
    case "Stats":
      return { ...next, stats: { pingMs: payload.ping_ms, relayed: payload.relayed } };
    case "PeerLeft":
      return { ...next, notice: `${payload.nick} left the game` };
    case "Desync":
      return { ...next, notice: "Desync detected — the host can resync from the game menu" };
    default:
      return next;
  }
}

/** On session end: keep the dialog only to show why the session ended early. */
export function lobbyAfterSessionEnd(lobby: NetplayLobby | null): NetplayLobby | null {
  if (!lobby || lobby.opening) return lobby;
  return lobby.phase.kind === "ended" && lobby.phase.reason.kind !== "Quit" ? lobby : null;
}

export function useNetplayLobby() {
  return useQuery<NetplayLobby | null>({
    queryKey: queryKeys.netplayLobby,
    queryFn: () => null,
    staleTime: Infinity,
  });
}

/** Updater of the cached lobby (`null` closes the dialog). */
export function useSetNetplayLobby() {
  const queryClient = useQueryClient();
  return (update: (lobby: NetplayLobby | null) => NetplayLobby | null) =>
    queryClient.setQueryData<NetplayLobby | null>(queryKeys.netplayLobby, (lobby) => update(lobby ?? null));
}

/** Folds runner netplay events into the lobby (subscribed once by the shell). */
export function subscribeNetplayEvents(queryClient: QueryClient): Promise<Unlisten> {
  return listen<NetplayEventPayload>(EVENTS.netplayEvent, (e) => {
    queryClient.setQueryData<NetplayLobby | null>(queryKeys.netplayLobby, (lobby) => foldNetplayEvent(lobby ?? null, e.payload));
  });
}

/** Session end: closes the lobby unless it explains an early end. `true` when the
 *  session was online (its error toast is then redundant with the dialog). */
export function endNetplayLobby(queryClient: QueryClient): boolean {
  const lobby = queryClient.getQueryData<NetplayLobby | null>(queryKeys.netplayLobby) ?? null;
  queryClient.setQueryData(queryKeys.netplayLobby, lobbyAfterSessionEnd(lobby));
  return lobby !== null;
}
