import { useEffect, useRef } from "react";
import type { CloudConflict, ConflictChoice } from "../api/tauri-commands-cloud";
import { useGameDetail } from "../hooks/use-game-detail-queries";
import { cloudFileLabels } from "../lib/cloud-file-labels";
import { relativeTimeInSentence } from "../lib/relative-time";

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold disabled:cursor-default disabled:opacity-50";

interface CloudConflictDialogProps {
  gameId: number;
  /** Null while the details load (chip path). */
  conflict: CloudConflict | null;
  /** Third button: "Play without syncing" (Play path) or "Decide later". */
  skipLabel: string;
  onChoose: (choice: ConflictChoice) => void;
  onSkip: () => void;
  /** Escape (Play path: cancels the Play). */
  onClose: () => void;
}

/** Native modal "Saves differ": this PC's vs the cloud's copy (writer + age), the
 *  conflicted files by friendly name, Keep this PC / Use cloud / skip. Neither choice
 *  loses data: the other copy goes to a backup. Focus starts on Keep this PC. */
export function CloudConflictDialog({ gameId, conflict, skipLabel, onChoose, onSkip, onClose }: CloudConflictDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const { data: game } = useGameDetail(gameId);
  const loading = conflict === null;

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);
  useEffect(() => {
    if (!loading) keepRef.current?.focus();
  }, [loading]);

  const when = (time: number | null) => (time !== null ? relativeTimeInSentence(time) : "at an unknown time");
  const device = conflict?.remoteDevice ? `${conflict.remoteDevice}, ` : "";

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="cloud-conflict-title"
      aria-busy={loading}
      className="m-auto w-[min(500px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="cloud-conflict-title" className="m-0 font-display text-[22px] font-semibold">
          Saves differ{game ? ` — ${game.title}` : ""}
        </h2>
        {loading ? (
          <p role="status" className="m-0 text-text-soft">Reading the cloud saves…</p>
        ) : (
          <>
            <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
              <dt className="font-bold">This PC</dt>
              <dd className="m-0 text-text-soft">saved {when(conflict.localTime)}</dd>
              <dt className="font-bold">Cloud</dt>
              <dd className="m-0 text-text-soft">
                {device}
                {when(conflict.remoteTime)}
              </dd>
            </dl>
            <div className="flex flex-col gap-1">
              <p className="m-0 text-sm text-text-soft">Changed on both sides:</p>
              <ul className="m-0 flex list-disc flex-col gap-0.5 pl-5 text-sm">
                {cloudFileLabels(conflict.files).map((label) => (
                  <li key={label}>{label}</li>
                ))}
              </ul>
            </div>
            <p className="m-0 text-xs text-muted">The other copy is kept in a backup.</p>
          </>
        )}
        <div className="flex flex-wrap justify-end gap-3">
          <button type="button" onClick={onSkip} className={`${BUTTON} border-0 bg-transparent font-semibold text-text-soft hover:text-white`}>
            {skipLabel}
          </button>
          <button
            type="button"
            disabled={loading}
            onClick={() => onChoose("useRemote")}
            className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white`}
          >
            Use cloud
          </button>
          <button
            ref={keepRef}
            type="button"
            disabled={loading}
            onClick={() => onChoose("keepLocal")}
            className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}
          >
            Keep this PC
          </button>
        </div>
      </div>
    </dialog>
  );
}
