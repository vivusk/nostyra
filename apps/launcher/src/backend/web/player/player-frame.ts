// The player iframe, launcher side: one same-origin `player.html` per game, full
// window above the launcher (its own React controls sit above it), removed on exit
// so the browser frees the emulator's memory (phase 00: ~6 GB for a big PS1 disc
// back to ~80 MB). Messages are typed in player-messages.ts.
import type { FromPlayer, PlayerBoot, ToPlayer } from "./player-messages";

/** Below the launcher's toasts (z-50) and player controls, above every page. */
const FRAME_Z_INDEX = "45";

export interface PlayerFrame {
  send(message: ToPlayer): void;
  /** Detaches the iframe (the emulator stops with it). */
  remove(): void;
}

/** Opens the player, posts `boot` once it is ready, and hands every later message to
 *  `onMessage`. */
export function openPlayerFrame(boot: PlayerBoot, onMessage: (message: FromPlayer) => void): PlayerFrame {
  const frame = document.createElement("iframe");
  frame.src = `${import.meta.env.BASE_URL}player.html`;
  frame.title = "Game";
  frame.allow = "fullscreen; gamepad; autoplay";
  Object.assign(frame.style, {
    position: "fixed",
    inset: "0",
    width: "100%",
    height: "100%",
    border: "0",
    background: "#000",
    zIndex: FRAME_Z_INDEX,
  });
  const send = (message: ToPlayer) => frame.contentWindow?.postMessage(message, location.origin);
  const listener = (event: MessageEvent) => {
    if (event.source !== frame.contentWindow || event.origin !== location.origin) return;
    const message = event.data as (FromPlayer & { from?: string }) | null;
    if (message?.from !== "player") return;
    if (message.type === "ready") send({ type: "boot", boot });
    else onMessage(message);
  };
  // Keyboard focus belongs inside the player. Focusing the <iframe> element before it
  // loads is not enough everywhere (Safari / macOS leaves the keys on this page), so
  // the frame's window is focused once loaded, and any key that still lands on this
  // page (focus lost to the page, e.g. after clicking outside the canvas) is forwarded
  // to the player and focus handed back. An open dialog (slot picker) keeps its keys.
  const focusPlayer = () => frame.contentWindow?.focus();
  const bound = new Set(["Escape", ...Object.keys(boot.input.keys), ...Object.values(boot.input.hotkeys.keyboard).flat()]);
  const forwardKey = (event: KeyboardEvent) => {
    if (document.querySelector("dialog[open]") || !bound.has(event.code)) return;
    event.preventDefault();
    event.stopPropagation();
    send({ type: "key", code: event.code, down: event.type === "keydown", repeat: event.repeat });
    focusPlayer();
  };
  frame.addEventListener("load", focusPlayer);
  window.addEventListener("message", listener);
  window.addEventListener("keydown", forwardKey, true);
  window.addEventListener("keyup", forwardKey, true);
  document.body.appendChild(frame);
  frame.focus();
  return {
    send,
    remove() {
      window.removeEventListener("message", listener);
      window.removeEventListener("keydown", forwardKey, true);
      window.removeEventListener("keyup", forwardKey, true);
      frame.remove();
    },
  };
}
