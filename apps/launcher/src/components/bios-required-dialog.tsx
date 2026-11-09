import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../api/query-keys";
import { useAddBiosFile, useBiosStatus } from "../hooks/use-bios-queries";
import { usePlayGame } from "../hooks/use-play-game";
import { useNavigation } from "../navigation/navigation-context";

/** Play was refused because the core has no HLE BIOS and no valid dump is in its
 *  BIOS folder (`bios_missing`, PS2). */
export interface BiosRequiredPrompt {
  coreId: string;
  gameId: number;
  message: string;
}

/** Offers the BIOS file picker (same import as Settings → BIOS), then plays the game
 *  once a file was added; "Settings" opens the BIOS section instead. Opened by
 *  `useLaunchGame` through the `biosRequiredPrompt` query; rendered once by `AppShell`
 *  inside `NavigationProvider` (`ResumeFlowProvider` sits outside it, and a hook
 *  without its provider would unmount the whole app). */
export function BiosRequiredDialog() {
  const queryClient = useQueryClient();
  const { data: prompt } = useQuery<BiosRequiredPrompt | null>({
    queryKey: queryKeys.biosRequiredPrompt,
    queryFn: () => null,
    staleTime: Infinity,
  });
  if (!prompt) return null;
  return <PromptDialog prompt={prompt} onClose={() => queryClient.setQueryData(queryKeys.biosRequiredPrompt, null)} />;
}

function PromptDialog({ prompt, onClose }: { prompt: BiosRequiredPrompt; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const { data: cores } = useBiosStatus();
  const addBios = useAddBiosFile();
  const play = usePlayGame();
  const { navigate } = useNavigation();
  const core = cores?.find((c) => c.coreId === prompt.coreId);
  const systems = core?.systemNames.join(", ") ?? "This system";

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  const chooseFile = async () => {
    if (!core) return;
    // null = picker cancelled (stay open); the import's own errors are toasted.
    const added = await addBios.mutateAsync(core).catch(() => null);
    if (!added) return;
    onClose();
    play(prompt.gameId);
  };

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="bios-required-title"
      className="m-auto w-[min(480px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="bios-required-title" className="m-0 font-display text-[22px] font-semibold">
          {systems} BIOS needed
        </h2>
        <p className="m-0 leading-relaxed text-text-soft">{prompt.message}</p>
        {core && <p className="m-0 text-xs break-all text-muted">Folder: {core.folder}</p>}
        <p className="m-0 text-xs text-muted">
          Nostyra does not include or download BIOS files. Use a BIOS dumped from your own console.
        </p>
        <div className="flex flex-wrap justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white"
          >
            Close
          </button>
          <button
            type="button"
            onClick={() => {
              onClose();
              navigate({ page: "settings" });
            }}
            className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white"
          >
            Settings
          </button>
          <button
            type="button"
            disabled={!core || addBios.isPending}
            onClick={() => void chooseFile()}
            className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
          >
            Choose BIOS file…
          </button>
        </div>
      </div>
    </dialog>
  );
}
