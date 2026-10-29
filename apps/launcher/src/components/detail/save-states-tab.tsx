import { useState } from "react";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import type { StateSlotId } from "../../api/tauri-commands-gameplay";
import { saveSlotLabel } from "../../lib/save-slot-label";
import { useGameSession, useLaunchGame } from "../../hooks/use-game-session";
import { useDeleteSaveState, useSaveStates } from "../../hooks/use-save-state-queries";
import { ConfirmDialog } from "../confirm-dialog";
import { MemoryCardsPanel } from "./memory-cards-panel";
import { SaveStateCard } from "./save-state-card";

/** Detail → Save states: the resume point card first (written on quit; Continue plays
 *  from it), then the used slots (the runner writes them; the list refreshes when a
 *  session ends). Load = play from that slot; Delete asks first. */
export function SaveStatesTab({ game }: { game: GameDetail }) {
  const { data: states, isError } = useSaveStates(game.id);
  const { data: session } = useGameSession();
  const { mutate: launch } = useLaunchGame();
  const deleteState = useDeleteSaveState();
  const [confirmSlot, setConfirmSlot] = useState<StateSlotId | null>(null);

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
      <h2 id="save-states-title" className="m-0 font-display text-[22px] font-semibold">
        Save states
      </h2>
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
              onLoad={() => launch({ gameId: game.id, loadState: state.slot })}
              onDelete={() => setConfirmSlot(state.slot)}
            />
          ))}
        </div>
      )}
      <MemoryCardsPanel gameId={game.id} running={running} missing={game.missing} />
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
