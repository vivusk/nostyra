import { useEffect, useRef } from "react";
import type { ResumeInfo, ResumeWarning } from "../api/tauri-commands-gameplay";
import { useGameDetail } from "../hooks/use-game-detail-queries";
import { assetUrl } from "../lib/asset-url";
import { relativeTimeInSentence } from "../lib/relative-time";

const WARNING_TEXT: Record<ResumeWarning, string> = {
  "sram-newer": "Your in-game save is newer than this resume point",
  "patch-changed": "The ROM patch changed",
  "core-version-changed": "Core version changed",
  "state-incompatible": "Made by another core version",
};

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold";

interface ResumeDialogProps {
  gameId: number;
  info: ResumeInfo;
  onContinue: () => void;
  onStartFresh: () => void;
  /** Cancel, Escape, or after either choice. */
  onClose: () => void;
}

/** Native modal "Continue where you left off?": resume-point thumbnail + age, warnings,
 *  Continue / Start fresh. Focus starts on Continue, or on Start fresh when there are
 *  warnings (a resume would revert newer progress). A resume point the runner would
 *  refuse (`state-incompatible`) offers Start fresh only. Escape cancels. */
export function ResumeDialog({ gameId, info, onContinue, onStartFresh, onClose }: ResumeDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const continueRef = useRef<HTMLButtonElement>(null);
  const freshRef = useRef<HTMLButtonElement>(null);
  const { data: game } = useGameDetail(gameId);
  const hasWarnings = info.warnings.length > 0;
  const incompatible = info.warnings.includes("state-incompatible");
  const warningText = (warning: ResumeWarning) =>
    warning === "state-incompatible"
      ? `${info.incompatibleReason ?? WARNING_TEXT[warning]} — cannot load`
      : WARNING_TEXT[warning];
  const thumbnail = assetUrl(info.thumbnailPath, info.createdAt);
  const savedAt = info.createdAt !== null ? relativeTimeInSentence(info.createdAt) : null;

  useEffect(() => {
    dialogRef.current?.showModal();
    (hasWarnings ? freshRef : continueRef).current?.focus();
  }, [hasWarnings]);

  const choose = (action: () => void) => () => {
    action();
    onClose();
  };

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="resume-dialog-title"
      className="m-auto w-[min(480px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="resume-dialog-title" className="m-0 font-display text-[22px] font-semibold">
          Continue {game?.title ?? "this game"}?
        </h2>
        <div className="flex aspect-[8/7] items-center justify-center overflow-hidden rounded-card bg-black">
          {thumbnail ? (
            <img src={thumbnail} alt="Where you left off" className="size-full object-contain [image-rendering:pixelated]" />
          ) : (
            <span className="text-sm text-muted">No preview</span>
          )}
        </div>
        {savedAt && <p className="m-0 text-sm text-text-soft">Resume point saved {savedAt}.</p>}
        {hasWarnings && (
          <ul role="alert" className="m-0 flex list-none flex-col gap-1 p-0 text-sm text-warning">
            {info.warnings.map((warning) => (
              <li key={warning}>{warningText(warning)}</li>
            ))}
          </ul>
        )}
        <div className="flex flex-wrap items-center justify-end gap-3">
          <button type="button" onClick={onClose} className={`${BUTTON} mr-auto border-0 bg-transparent px-4 font-semibold text-text-soft hover:text-white`}>
            Cancel
          </button>
          <button
            ref={freshRef}
            type="button"
            onClick={choose(onStartFresh)}
            className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white`}
          >
            Start fresh
          </button>
          {!incompatible && (
            <button ref={continueRef} type="button" onClick={choose(onContinue)} className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}>
              Continue
            </button>
          )}
        </div>
        <p className="m-0 text-xs text-muted">Start fresh boots the game normally. Your in-game saves are kept.</p>
      </div>
    </dialog>
  );
}
