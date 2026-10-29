import { useCallback, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { gameplayCommands, type ResumeInfo } from "../api/tauri-commands-gameplay";
import { queryKeys } from "../api/query-keys";
import { useLaunchGame } from "../hooks/use-game-session";
import { PlayGameContext, resumeDecision } from "../hooks/use-play-game";
import { CoreInstallDialog } from "./core-install-dialog";
import { ResumeDialog } from "./resume-dialog";

/** Provides `usePlayGame()` and renders the Resume dialog (Continue / Start fresh) and
 *  the missing-core install prompt once for the whole app, so every Play entry point
 *  behaves the same. */
export function ResumeFlowProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const { mutate: launch } = useLaunchGame();
  const [asking, setAsking] = useState<{ gameId: number; info: ResumeInfo } | null>(null);
  // One decision at a time: a double click must not launch twice.
  const deciding = useRef(false);

  const play = useCallback(
    async (gameId: number) => {
      if (deciding.current) return;
      deciding.current = true;
      try {
        // Always fresh from disk: the resume point changes when a session ends.
        const info = await queryClient.fetchQuery({
          queryKey: queryKeys.resumeInfo(gameId),
          queryFn: () => gameplayCommands.getResumeInfo(gameId),
          staleTime: 0,
        });
        const decision = resumeDecision(info);
        if (decision === "fresh") launch(gameId);
        else if (decision === "continue") launch({ gameId, loadState: "auto" });
        else setAsking({ gameId, info });
      } catch (error) {
        // No resume info (e.g. no ROM to key saves on): launch normally; play_game
        // reports any real problem.
        console.warn("get_resume_info failed; launching without the resume point", error);
        launch(gameId);
      } finally {
        deciding.current = false;
      }
    },
    [queryClient, launch],
  );
  const playGame = useCallback((gameId: number) => void play(gameId), [play]);

  return (
    <PlayGameContext.Provider value={playGame}>
      {children}
      <CoreInstallDialog />
      {asking && (
        <ResumeDialog
          gameId={asking.gameId}
          info={asking.info}
          onContinue={() => launch({ gameId: asking.gameId, loadState: "auto" })}
          onStartFresh={() => launch(asking.gameId)}
          onClose={() => setAsking(null)}
        />
      )}
    </PlayGameContext.Provider>
  );
}
