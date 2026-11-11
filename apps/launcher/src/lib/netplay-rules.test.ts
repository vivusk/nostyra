// node --test: Play online rules (code clean-up, roster order, status text, web gate).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  formatRoomCode,
  lobbyStatusText,
  nextLobbyPhase,
  normaliseRoomCode,
  playerChoices,
  playOnlineState,
  roomErrorText,
  sortRoster,
  type LobbyPhase,
} from "./netplay-rules.ts";

test("room codes: spaces / dashes dropped, uppercase, look-alikes mapped", () => {
  assert.equal(normaliseRoomCode(" k7q-2zp "), "K7Q2ZP");
  assert.equal(normaliseRoomCode("oil 123"), "011123");
  assert.equal(normaliseRoomCode("K7Q2Z"), null);
  assert.equal(normaliseRoomCode("K7Q2ZPX"), null);
  assert.equal(normaliseRoomCode("K7Q2Z?"), null);
  assert.equal(normaliseRoomCode("K7Q2ZU"), null, "U is not in the alphabet");
  assert.equal(formatRoomCode("K7Q2ZP"), "K7Q-2ZP");
});

test("roster: host first, then players, then spectators, join order kept", () => {
  const roster = [
    { nick: "S1", role: "spectator" as const },
    { nick: "P1", role: "player" as const },
    { nick: "H", role: "host" as const },
    { nick: "P2", role: "player" as const },
  ];
  assert.deepEqual(
    sortRoster(roster).map((e) => e.nick),
    ["H", "P1", "P2", "S1"],
  );
});

test("status text follows the runner events", () => {
  let phase: LobbyPhase = { kind: "connecting" };
  assert.equal(lobbyStatusText(phase, false), "Connecting to the room…");
  phase = nextLobbyPhase(phase, { event: "Lobby" });
  assert.equal(lobbyStatusText(phase, false), "Waiting for host");
  assert.match(lobbyStatusText(phase, true), /press Start in the game window/);
  phase = nextLobbyPhase(phase, { event: "Syncing", percent: 40 });
  assert.equal(lobbyStatusText(phase, false), "Syncing 40 %");
  phase = nextLobbyPhase(phase, { event: "Stats" });
  assert.equal(phase.kind, "syncing", "stats change nothing");
  phase = nextLobbyPhase(phase, { event: "Started" });
  assert.equal(lobbyStatusText(phase, false), "Started");
  const refused = nextLobbyPhase(phase, {
    event: "Ended",
    reason: { kind: "Refused", mismatches: ["Different core version: 1.62 vs 1.63"] },
  });
  assert.equal(lobbyStatusText(refused, false), "Refused: Different core version: 1.62 vs 1.63");
  const after = nextLobbyPhase(refused, { event: "Lobby" });
  assert.equal(after, refused, "an ended session stays ended");
  const hostLeft = nextLobbyPhase({ kind: "started" }, { event: "Ended", reason: { kind: "HostLeft" } });
  assert.equal(lobbyStatusText(hostLeft, false), "Ended: the host left");
});

test("join refusals keep the backend text (409 → started / full)", () => {
  assert.equal(roomErrorText({ kind: "netplay", message: "Game already started" }), "Game already started");
  assert.equal(roomErrorText({ kind: "netplay", message: "Room is full" }), "Room is full");
  assert.equal(roomErrorText({ kind: "session", message: "A game is already running." }), "Could not open the room: A game is already running.");
  assert.equal(roomErrorText(undefined), "Could not open the room.");
});

test("web edition (capability off) shows Play online as a desktop-only notice", () => {
  const game = { netplay: { support: "yes", offline: false }, missing: false };
  const hidden = { visible: false, disabledReason: null, desktopOnly: false };
  assert.deepEqual(playOnlineState(false, game, false), { visible: true, disabledReason: null, desktopOnly: true });
  assert.deepEqual(playOnlineState(false, { ...game, missing: true }, true), { visible: true, disabledReason: null, desktopOnly: true }, "the notice is never disabled");
  assert.deepEqual(playOnlineState(false, { ...game, netplay: { support: "no", offline: false } }, false), hidden);
  assert.deepEqual(playOnlineState(false, { missing: false }, false), hidden);
  assert.deepEqual(playOnlineState(true, { missing: false }, false), hidden);
  assert.deepEqual(playOnlineState(true, game, false), { visible: true, disabledReason: null, desktopOnly: false });
  assert.equal(playOnlineState(true, { ...game, netplay: { support: "experimental", offline: true } }, false).disabledReason, "Online play is off (offline mode)");
  assert.equal(playOnlineState(true, { ...game, netplay: { support: "no", offline: false } }, false).visible, false);
  assert.equal(playOnlineState(true, game, true).disabledReason, "A game is already running");
});

test("player picker follows the room cap", () => {
  assert.deepEqual(playerChoices(4), [2, 3, 4]);
  assert.deepEqual(playerChoices(2), [2]);
  assert.deepEqual(playerChoices(1), [2]);
});
