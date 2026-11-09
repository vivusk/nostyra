import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../api/query-keys";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import { gameplayCommands, type StateSlotId } from "../../api/tauri-commands-gameplay";
import { saveSlotLabel } from "../../lib/save-slot-label";
import { useGameSession, useLaunchGame } from "../../hooks/use-game-session";
import { useDeleteSaveState, useSaveStates } from "../../hooks/use-save-state-queries";
import { useSyncBeforePlay } from "../../hooks/use-cloud-pre-play";
import { CloudSyncChip } from "./cloud-sync-chip";
import { ConfirmDialog } from "../confirm-dialog";
import { useErrorToast } from "../error-toast";
import { capabilities } from "../../backend";
import { MemoryCardsPanel } from "./memory-cards-panel";
import { SaveStateCard } from "./save-state-card";

/** Detail → Save states: the resume point card first (written on quit; Continue plays
 *  from it), then the used slots (the runner writes them; the list refreshes when a
 *  session ends). Load = play from that slot, after the cloud pre-play pull (desktop,
 *  signed in) and a refreshed list; Delete asks first. The cloud chip sits by the title. */
export function SaveStatesTab({ game }: { game: GameDetail }) {
  const { data: states, isError } = useSaveStates(game.id);
  const { data: session } = useGameSession();
  const { mutate: launch } = useLaunchGame();
  const deleteState = useDeleteSaveState();
  const [confirmSlot, setConfirmSlot] = useState<StateSlotId | null>(null);
  const queryClient = useQueryClient();
  const syncBeforePlay = useSyncBeforePlay();
  const { showNotice } = useErrorToast();
  // One load at a time: the cloud step may take a few seconds.
  const loading = useRef(false);

  const loadSlot = async (slot: StateSlotId) => {
    if (loading.current) return;
    loading.current = true;
    try {
      if (!(await syncBeforePlay(game.id))) return;
      // The pull may have replaced or removed the slot: launch from the fresh list.
      const fresh = await queryClient.fetchQuery({
        queryKey: queryKeys.saveStates(game.id),
        queryFn: () => gameplayCommands.listSaveStates(game.id),
        staleTime: 0,
      });
      if (fresh.some((state) => state.slot === slot)) launch({ gameId: game.id, loadState: slot });
      else showNotice(`${saveSlotLabel(slot)} was removed by the cloud sync`);
    } catch (error) {
      console.warn("save states not refreshed; loading the slot", error);
      launch({ gameId: game.id, loadState: slot });
    } finally {
      loading.current = false;
    }
  };

  const running = session?.gameId === game.id;
  // Same rules as the hero Play button.
  const loadDisabledReason = game.missing
    ? "The ROM file is missing"
    : running ? "The game is running"
    : session ? "Another game is running"
    : null;
  const deleteDisabledReason = running ? "Close the game first: it is running right now" : null;

  if (!states) return isError ? <p className="m-0 text-text-soft">The save states could not be loaded.</p> : null;

  return (
    <section aria-labelledby="save-states-title" className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center gap-3">
        <h2 id="save-states-title" className="m-0 font-display text-[22px] font-semibold">
          Save states
        </h2>
        {capabilities.cloudSaves && <CloudSyncChip gameId={game.id} />}
      </div>
      {states.length === 0 ? (
        <p className="m-0 max-w-[720px] rounded-[14px] border border-dashed border-[#33333D] bg-surface p-7 leading-relaxed text-text-soft">
          No save states yet. Press F1 in-game or use the pause menu (Esc).
        </p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-5">
          {states.map((state) => (
            <SaveStateCard
              key={state.slot}
              state={state}
              loadDisabledReason={loadDisabledReason}
              deleteDisabledReason={deleteDisabledReason}
              onLoad={() => void loadSlot(state.slot)}
              onDelete={() => setConfirmSlot(state.slot)}
            />
          ))}
        </div>
      )}
      {capabilities.memoryCards && <MemoryCardsPanel gameId={game.id} running={running} missing={game.missing} />}
      {confirmSlot !== null && (
        <ConfirmDialog
          title={`Delete ${saveSlotLabel(confirmSlot).toLowerCase()}?`}
          confirmLabel="Delete"
          danger
          onConfirm={() => deleteState.mutate({ gameId: game.id, slot: confirmSlot })}
          onClose={() => setConfirmSlot(null)}
        >
          {confirmSlot === "auto"
            ? "The next Play starts the game fresh. A new resume point is written when you quit a game."
            : "The save state and its thumbnail are removed from disk. This cannot be undone."}
        </ConfirmDialog>
      )}
    </section>
  );
}
