import { useCallback, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { gameplayCommands, type ResumeInfo } from "../api/tauri-commands-gameplay";
import { queryKeys } from "../api/query-keys";
import { capabilities } from "../backend";
import { useLaunchGame } from "../hooks/use-game-session";
import { PlayGameContext, resumeDecision } from "../hooks/use-play-game";
import { CloudPrePlayContext, useCloudPrePlayState } from "../hooks/use-cloud-pre-play";
import { CloudConflictDialog } from "./cloud-conflict-dialog";
import { CloudDialogsHost } from "./cloud-dialogs-host";
import { CoreInstallDialog } from "./core-install-dialog";
import { ResumeDialog } from "./resume-dialog";

/** Provides `usePlayGame()` and renders the Resume dialog (Continue / Start fresh) and
 *  the missing-core install prompt once for the whole app, so every Play entry point
 *  behaves the same. (The missing-BIOS prompt lives in `AppShell`: it needs navigation.)
 *  Cloud saves (desktop): the pre-play pull + its conflict dialog run first, then the
 *  resume decision on the resulting files; also hosts the other cloud dialogs. */
export function ResumeFlowProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const { mutate: launch } = useLaunchGame();
  const { syncBeforePlay, pending: cloudConflict } = useCloudPrePlayState();
  const [asking, setAsking] = useState<{ gameId: number; info: ResumeInfo } | null>(null);
  // One decision at a time: a double click must not launch twice.
  const deciding = useRef(false);

  const play = useCallback(
    async (gameId: number) => {
      if (deciding.current) return;
      deciding.current = true;
      try {
        if (!(await syncBeforePlay(gameId))) return; // conflict dialog cancelled
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
    [queryClient, launch, syncBeforePlay],
  );
  const playGame = useCallback((gameId: number) => void play(gameId), [play]);

  return (
    <PlayGameContext.Provider value={playGame}>
      <CloudPrePlayContext.Provider value={syncBeforePlay}>{children}</CloudPrePlayContext.Provider>
      <CoreInstallDialog />
      {capabilities.cloudSaves && <CloudDialogsHost />}
      {cloudConflict && (
        <CloudConflictDialog
          gameId={cloudConflict.gameId}
          conflict={cloudConflict.conflict}
          skipLabel="Play without syncing"
          onChoose={cloudConflict.answer}
          onSkip={() => cloudConflict.answer("skip")}
          onClose={() => cloudConflict.answer("cancel")}
        />
      )}
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
