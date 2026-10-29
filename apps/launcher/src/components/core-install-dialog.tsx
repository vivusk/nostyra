import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../api/query-keys";
import { useCoreStatus } from "../hooks/use-game-session";
import { useSetupState } from "../hooks/use-library-queries";
import { usePlayGame } from "../hooks/use-play-game";
import { CoreDownloadPanel } from "./core-download-panel";

/** Play was refused because the game's core is not installed (`core_missing`). */
export interface CoreInstallPrompt {
  coreId: string;
  gameId: number;
  message: string;
}

/** Offers the missing core (download with progress), then Play once it is installed.
 *  Opened by `useLaunchGame` through the `coreInstallPrompt` query; rendered once by
 *  `ResumeFlowProvider`. */
export function CoreInstallDialog() {
  const queryClient = useQueryClient();
  const { data: prompt } = useQuery<CoreInstallPrompt | null>({
    queryKey: queryKeys.coreInstallPrompt,
    queryFn: () => null,
    staleTime: Infinity,
  });
  if (!prompt) return null;
  return <PromptDialog prompt={prompt} onClose={() => queryClient.setQueryData(queryKeys.coreInstallPrompt, null)} />;
}

function PromptDialog({ prompt, onClose }: { prompt: CoreInstallPrompt; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const { data: setup } = useSetupState();
  const { data: status } = useCoreStatus(prompt.coreId);
  const play = usePlayGame();
  const core = setup?.cores.find((c) => c.id === prompt.coreId);

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="core-install-title"
      className="m-auto w-[min(480px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="core-install-title" className="m-0 font-display text-[22px] font-semibold">
          Install {core?.displayName ?? "the emulator core"}?
        </h2>
        <p className="m-0 leading-relaxed text-text-soft">{prompt.message}</p>
        {core && <CoreDownloadPanel core={core} />}
        <div className="flex justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white"
          >
            Close
          </button>
          <button
            type="button"
            disabled={!status?.installed}
            onClick={() => {
              onClose();
              play(prompt.gameId);
            }}
            className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
          >
            Play
          </button>
        </div>
      </div>
    </dialog>
  );
}
