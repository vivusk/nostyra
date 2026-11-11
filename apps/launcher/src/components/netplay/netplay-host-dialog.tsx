import { useEffect, useRef, useState } from "react";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import { usePlayGame } from "../../hooks/use-play-game";
import { playerChoices, SPECTATOR_WARN_ABOVE } from "../../lib/netplay-rules";
import { SegmentedControl } from "../gameplay/gameplay-setting-controls";

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold";

/** Host game: players (2 – the system's room cap) and spectators, then the Play path
 *  (cloud pull + resume choice) opens the room. The host starts the game from the
 *  game window (Start / Enter) once everyone is in. */
export function NetplayHostDialog({ game, onClose }: { game: GameDetail; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const playGame = usePlayGame();
  const choices = playerChoices(game.netplay?.maxPlayers ?? 2);
  const [players, setPlayers] = useState(choices[0]);
  const [spectators, setSpectators] = useState(true);
  const experimental = game.netplay?.support === "experimental";

  useEffect(() => dialogRef.current?.showModal(), []);

  const host = () => {
    playGame(game.id, { players, allowSpectators: spectators });
    onClose();
  };

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="netplay-host-title"
      className="m-auto w-[min(480px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="netplay-host-title" className="m-0 font-display text-[22px] font-semibold">
          Host {game.title}
        </h2>
        <div className="flex flex-col gap-2">
          <span id="netplay-players-label" className="text-sm font-semibold">
            Players
          </span>
          <SegmentedControl
            controlId="netplay-players"
            choices={choices.map((value) => ({ value, label: String(value) }))}
            value={players}
            onChange={(value) => value !== null && setPlayers(value)}
          />
          {choices.length === 1 && <span className="text-xs text-muted">This system has one controller per console: others can watch.</span>}
        </div>
        <label className="flex cursor-pointer items-start gap-3 text-sm text-text-soft">
          <input
            type="checkbox"
            checked={spectators}
            onChange={(e) => setSpectators(e.target.checked)}
            className="mt-0.5 h-4 w-4 accent-[var(--color-accent)]"
          />
          <span className="flex flex-col">
            Allow spectators
            <span className="text-xs text-muted">
              Up to 8; more than {SPECTATOR_WARN_ABOVE} may slow your connection (each one streams from your PC).
            </span>
          </span>
        </label>
        {experimental && (
          <p role="note" className="m-0 rounded-control border border-warning/40 bg-surface-raised px-3 py-2 text-sm text-text-soft">
            Experimental: online play for this system may desync. The host can resync from the game menu.
          </p>
        )}
        <p className="m-0 text-xs text-muted">
          Everyone needs the same game file and Nostyra version. Your room code appears next; press Start in
          the game window when everyone has joined.
        </p>
        <div className="flex justify-end gap-3">
          <button type="button" onClick={onClose} className={`${BUTTON} border-0 bg-transparent text-text-soft hover:text-white`}>
            Cancel
          </button>
          <button type="button" autoFocus onClick={host} className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}>
            Host
          </button>
        </div>
      </div>
    </dialog>
  );
}
