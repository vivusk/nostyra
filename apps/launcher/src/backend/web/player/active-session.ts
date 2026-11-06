// The one web game session at a time (desktop `AppState.session`): Play is refused
// while a game starts or runs; the slot is freed when the session ends.
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";
import { commandError } from "../web-command-table";
import { preparePlay } from "./play-launch";
import { PlaySession } from "./play-session";

let session: PlaySession | null = null;
let startingGameId: number | null = null;

/** Game starting or running, if any. */
export function activeGameId(): number | null {
  return session?.gameId ?? startingGameId;
}

/** `play_game`: prepares everything, then opens the player (returns once it is open). */
export async function startSession(gameId: number, loadState: StateSlotId | null): Promise<void> {
  if (activeGameId() !== null) throw commandError("session", "A game is already running.");
  startingGameId = gameId;
  try {
    const launch = await preparePlay(gameId, loadState);
    const current: PlaySession = new PlaySession(launch, () => {
      if (session === current) session = null;
    });
    session = current;
  } finally {
    startingGameId = null;
  }
}

/** The player's Exit button. */
export function exitSession(): void {
  session?.exit();
}

/** `busy` like the desktop when `gameId` is the running game. */
export function ensureIdle(gameId: number): void {
  if (activeGameId() === gameId) throw commandError("busy", "Close the game first: it is running right now.");
}
