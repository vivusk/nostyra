// The one Play path (Home hero, Library cards, Detail header): asks the backend for the
// game's resume point, then launches fresh, continues, or opens the Resume dialog.
// The state + dialog live in `ResumeFlowProvider` (shell level); this file holds the
// decision rule and the context the pages read.
import { createContext, useContext } from "react";
import type { ResumeInfo } from "../api/tauri-commands-gameplay";

/** What Play does with a game's resume info. */
export type ResumeDecision = "fresh" | "continue" | "ask";

/** `never` or no resume point → fresh; `always` without warnings → continue; otherwise
 *  ask (warnings force a conscious choice even in `always`). */
export function resumeDecision(info: ResumeInfo): ResumeDecision {
  if (info.mode === "never" || !info.available) return "fresh";
  if (info.mode === "always" && info.warnings.length === 0) return "continue";
  return "ask";
}

/** Starts the Play flow for a game (fire-and-forget; errors surface as toasts). */
export type PlayGame = (gameId: number) => void;

export const PlayGameContext = createContext<PlayGame>(() => {});

/** Play with the resume rules (`resume.mode`); needs `ResumeFlowProvider` above. */
export function usePlayGame(): PlayGame {
  return useContext(PlayGameContext);
}
