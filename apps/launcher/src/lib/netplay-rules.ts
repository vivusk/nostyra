// Pure rules of the Play online UI (import-free so `node --test` can load it): room
// code clean-up, roster order, lobby status text, when the Play online menu shows.

/** Crockford base32 as the server issues codes (no I, L, O, U). */
const CODE_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
export const ROOM_CODE_LENGTH = 6;

/** Spaces / dashes dropped, uppercase, look-alikes mapped (`O` → `0`, `I` / `L` → `1`);
 *  `null` unless the result is a 6-character code. */
export function normaliseRoomCode(input: string): string | null {
  const code = input
    .replace(/[\s-]/g, "")
    .toUpperCase()
    .replace(/O/g, "0")
    .replace(/[IL]/g, "1");
  if (code.length !== ROOM_CODE_LENGTH) return null;
  return [...code].every((c) => CODE_ALPHABET.includes(c)) ? code : null;
}

/** `K7Q2ZP` → `K7Q-2ZP` (easier to read aloud). */
export function formatRoomCode(code: string): string {
  return code.length === ROOM_CODE_LENGTH ? `${code.slice(0, 3)}-${code.slice(3)}` : code;
}

type Role = "host" | "player" | "spectator";
const ROLE_ORDER: Record<Role, number> = { host: 0, player: 1, spectator: 2 };

/** Host first, then players, then spectators (stable within a role: join order). */
export function sortRoster<T extends { role: Role }>(roster: readonly T[]): T[] {
  return roster
    .map((entry, index) => ({ entry, index }))
    .sort((a, b) => ROLE_ORDER[a.entry.role] - ROLE_ORDER[b.entry.role] || a.index - b.index)
    .map(({ entry }) => entry);
}

/** What the lobby dialog knows; folded from runner events (`foldLobbyEvent`). */
export type LobbyPhase =
  | { kind: "connecting" }
  | { kind: "waiting" }
  | { kind: "syncing"; percent: number }
  | { kind: "started" }
  | { kind: "ended"; reason: EndReason };

type EndReason =
  | { kind: "HostLeft" }
  | { kind: "Ineligible"; reason: string }
  | { kind: "Refused"; mismatches: string[] }
  | { kind: "Quit" }
  | { kind: "Error"; message: string };

/** The phase after one runner event (`event` tag as the runner sends it). */
export function nextLobbyPhase(phase: LobbyPhase, event: { event: string; percent?: number; reason?: EndReason }): LobbyPhase {
  if (phase.kind === "ended") return phase;
  switch (event.event) {
    case "Lobby":
      return phase.kind === "connecting" ? { kind: "waiting" } : phase;
    case "Syncing":
      return { kind: "syncing", percent: event.percent ?? 0 };
    case "Started":
      return { kind: "started" };
    case "Ended":
      return event.reason ? { kind: "ended", reason: event.reason } : phase;
    default:
      return phase;
  }
}

/** One status line for the dialog. `host`: the host waits for players, guests for the host. */
export function lobbyStatusText(phase: LobbyPhase, host: boolean): string {
  switch (phase.kind) {
    case "connecting":
      return "Connecting to the room…";
    case "waiting":
      return host ? "Waiting for players — press Start in the game window to begin" : "Waiting for host";
    case "syncing":
      return `Syncing ${phase.percent} %`;
    case "started":
      return "Started";
    case "ended":
      return endReasonText(phase.reason);
  }
}

export function endReasonText(reason: EndReason): string {
  switch (reason.kind) {
    case "HostLeft":
      return "Ended: the host left";
    case "Ineligible":
      return `Ended: ${reason.reason}`;
    case "Refused":
      return reason.mismatches.length > 0 ? `Refused: ${reason.mismatches.join("; ")}` : "Refused by the host";
    case "Quit":
      return "Ended";
    case "Error":
      return `Ended: ${reason.message}`;
  }
}

/** Text of a host / join refusal: netplay refusals ("Game already started", "Room is
 *  full", "You need … in your library") as the backend wrote them, others prefixed. */
export function roomErrorText(error: unknown): string {
  const { kind, message } = (error ?? {}) as { kind?: string; message?: string };
  const text = typeof message === "string" ? message.trim() : "";
  if (kind === "netplay" && text) return text;
  return text ? `Could not open the room: ${text}` : "Could not open the room.";
}

/** Where the web edition's "desktop app only" notice sends the user. */
export const DESKTOP_DOWNLOAD_URL = "https://nostyra.com/download/";

/** Whether Detail shows Play online, and why it is disabled (`null` = enabled).
 *  `desktopOnly` (web edition, capability off): the button stays enabled and opens a
 *  "desktop app only" notice instead of the Host / Join / Watch menu. */
export function playOnlineState(
  netplayCapability: boolean,
  game: { netplay?: { support: string; offline: boolean } | null; missing: boolean },
  sessionRunning: boolean,
): { visible: boolean; disabledReason: string | null; desktopOnly: boolean } {
  const support = game.netplay?.support ?? "no";
  if (support === "no") return { visible: false, disabledReason: null, desktopOnly: false };
  if (!netplayCapability) return { visible: true, disabledReason: null, desktopOnly: true };
  const enabled = (disabledReason: string | null) => ({ visible: true, disabledReason, desktopOnly: false });
  if (game.netplay?.offline) return enabled("Online play is off (offline mode)");
  if (game.missing) return enabled("The ROM file is missing");
  if (sessionRunning) return enabled("A game is already running");
  return enabled(null);
}

/** Host players picker: 2 ..= the system's room cap. */
export function playerChoices(maxPlayers: number): number[] {
  const cap = Math.max(2, Math.min(4, maxPlayers));
  return Array.from({ length: cap - 1 }, (_, i) => i + 2);
}

/** Spectators above this many load the host's upload: warn (server cap 8). */
export const SPECTATOR_WARN_ABOVE = 4;
