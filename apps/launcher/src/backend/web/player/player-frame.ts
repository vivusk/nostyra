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
  window.addEventListener("message", listener);
  document.body.appendChild(frame);
  frame.focus();
  return {
    send,
    remove() {
      window.removeEventListener("message", listener);
      frame.remove();
    },
  };
}
