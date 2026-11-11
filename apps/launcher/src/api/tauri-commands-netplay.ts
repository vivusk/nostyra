// Play online commands (src-tauri/src/commands/netplay_commands.rs). Desktop only:
// the web table rejects them (`backend/web/desktop-only`); the UI never calls them
// unless `capabilities.netplay`. Netplay settings are gameplay-config keys
// (`netplay.*`, global layer), written with `setGameplayConfigValue`.
import { invoke } from "../backend";
import type { StateSlotId } from "./tauri-commands-gameplay";

/** Seat a joiner asks for. */
export type NetplayJoinRole = "player" | "spectator";

/** `GET /v1/rooms/{code}` counts (lobby dialog poll before the runner connects). */
export interface NetplayRoomStatus {
  players: number;
  spectators: number;
  started: boolean;
  maxPlayers: number;
}

export interface NetplayHostRequest {
  gameId: number;
  players: number;
  allowSpectators: boolean;
  /** Chosen by the resume flow (host only: cloud pull + resume run first). */
  loadState?: StateSlotId;
}

export const netplayCommands = {
  /** Creates the room and starts the runner; resolves with the room code. */
  host: ({ gameId, players, allowSpectators, loadState }: NetplayHostRequest) =>
    invoke<{ code: string }>("netplay_host", { gameId, players, allowSpectators, loadState: loadState ?? null }),
  /** Joins by code; resolves with the local game id the runner plays. */
  join: (code: string, role: NetplayJoinRole) => invoke<number>("netplay_join", { code, role }),
  roomStatus: (code: string) => invoke<NetplayRoomStatus>("netplay_room_status", { code }),
  /** Lobby Cancel: ends the running game (before the online session started). */
  quitSession: () => invoke<void>("quit_session"),
  /** README Privacy section in the browser (Settings → Netplay). */
  openPrivacy: () => invoke<void>("open_netplay_privacy"),
};
