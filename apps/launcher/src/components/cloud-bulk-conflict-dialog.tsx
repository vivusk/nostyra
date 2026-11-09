import { useEffect, useRef, useState, type Ref } from "react";
import type { ConflictChoice } from "../api/tauri-commands-cloud";
import { useGameDetail } from "../hooks/use-game-detail-queries";

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold";

interface CloudBulkConflictDialogProps {
  /** Games still in conflict (≥ 1; opened for ≥ 2). */
  gameIds: number[];
  onChooseAll: (choice: ConflictChoice) => void;
  /** Opens one game's conflict dialog (this dialog comes back after it). */
  onPickGame: (gameId: number) => void;
  /** Escape / Decide later: the chips keep showing the conflicts. */
  onClose: () => void;
}

/** After Sync now / the startup pass left several conflicts: one answer for all
 *  (This PC / Cloud), or the per-game list. Focus starts on "This PC for all". */
export function CloudBulkConflictDialog({ gameIds, onChooseAll, onPickGame, onClose }: CloudBulkConflictDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const firstRef = useRef<HTMLButtonElement>(null);
  const [perGame, setPerGame] = useState(false);

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);
  useEffect(() => {
    firstRef.current?.focus();
  }, [perGame]);

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="cloud-bulk-title"
      className="m-auto w-[min(500px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="cloud-bulk-title" className="m-0 font-display text-[22px] font-semibold">
          Saves differ for {gameIds.length} {gameIds.length === 1 ? "game" : "games"}
        </h2>
        {perGame ? (
          <ul className="m-0 flex max-h-72 list-none flex-col gap-2 overflow-auto p-0">
            {gameIds.map((gameId, index) => (
              <GameRow key={gameId} gameId={gameId} buttonRef={index === 0 ? firstRef : undefined} onPick={onPickGame} />
            ))}
          </ul>
        ) : (
          <p className="m-0 leading-relaxed text-text-soft">
            These games were saved both on this PC and in the cloud since the last sync. Pick which copy to keep;
            the other copy is kept in a backup.
          </p>
        )}
        <div className="flex flex-wrap justify-end gap-3">
          <button type="button" onClick={onClose} className={`${BUTTON} border-0 bg-transparent font-semibold text-text-soft hover:text-white`}>
            Decide later
          </button>
          {!perGame && (
            <>
              <button
                type="button"
                onClick={() => setPerGame(true)}
                className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white`}
              >
                Decide per game
              </button>
              <button
                type="button"
                onClick={() => onChooseAll("useRemote")}
                className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white`}
              >
                Cloud for all
              </button>
              <button
                ref={firstRef}
                type="button"
                onClick={() => onChooseAll("keepLocal")}
                className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}
              >
                This PC for all
              </button>
            </>
          )}
        </div>
      </div>
    </dialog>
  );
}

function GameRow({ gameId, buttonRef, onPick }: { gameId: number; buttonRef?: Ref<HTMLButtonElement>; onPick: (gameId: number) => void }) {
  const { data: game } = useGameDetail(gameId);
  const title = game?.title ?? `Game ${gameId}`;
  return (
    <li className="flex items-center gap-3 rounded-control bg-surface-raised px-4 py-2 text-sm">
      <span className="flex-1">{title}</span>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => onPick(gameId)}
        aria-label={`Choose saves for ${title}`}
        className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white"
      >
        Choose…
      </button>
    </li>
  );
}
