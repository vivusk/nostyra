import type { SaveStateSlot } from "../../api/tauri-commands-gameplay";
import { assetUrl } from "../../lib/asset-url";
import { saveSlotLabel } from "../../lib/save-slot-label";
import { relativeTime, relativeTimeInSentence } from "../../lib/relative-time";

const BUTTON = "h-9 flex-1 cursor-pointer rounded-control font-body text-sm font-bold disabled:cursor-default disabled:opacity-50";

interface SaveStateCardProps {
  state: SaveStateSlot;
  /** Why Load is unavailable (game running, ROM missing); null = enabled. */
  loadDisabledReason: string | null;
  /** Why Delete is unavailable (this game is running); null = enabled. */
  deleteDisabledReason: string | null;
  onLoad: () => void;
  onDelete: () => void;
}

/** One used slot: native-size thumbnail letterboxed in a 4:3.5 frame, slot number,
 *  relative time (absolute on hover), Load + Delete. The resume point (`slot "auto"`)
 *  is the same card titled "Resume point" with an accent border and Continue. */
export function SaveStateCard({ state, loadDisabledReason, deleteDisabledReason, onLoad, onDelete }: SaveStateCardProps) {
  const thumbnail = assetUrl(state.thumbnailPath, state.createdAt);
  const savedAt = new Date(state.createdAt * 1000).toLocaleString();
  const label = saveSlotLabel(state.slot);
  const resume = state.slot === "auto";
  const loadLabel = resume ? "Continue" : "Load";
  // A state the runner would refuse cannot be loaded at all (Delete stays).
  const incompatible = state.incompatibleReason ? `${state.incompatibleReason} — cannot load` : null;
  const loadBlocked = incompatible ?? loadDisabledReason;

  return (
    <article className={`flex flex-col overflow-hidden rounded-card border bg-surface ${resume ? "border-accent/60" : "border-border"}`}>
      <div className="flex aspect-[4/3.5] items-center justify-center bg-black">
        {thumbnail ? (
          <img
            src={thumbnail}
            alt={`${label}, saved ${relativeTimeInSentence(state.createdAt)}`}
            className="size-full object-contain [image-rendering:pixelated]"
          />
        ) : (
          <span className="text-sm text-muted">No preview</span>
        )}
      </div>
      <div className="flex flex-col gap-3 p-4">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="m-0 font-display text-base font-semibold">{label}</h3>
          <time dateTime={new Date(state.createdAt * 1000).toISOString()} title={savedAt} className="text-xs text-muted">
            {relativeTime(state.createdAt)}
          </time>
        </div>
        {resume && <p className="-mt-2 mb-0 text-xs text-muted">Saved when you last quit</p>}
        {incompatible && <p className="-mt-2 mb-0 text-xs text-warning">{incompatible}</p>}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onLoad}
            disabled={loadBlocked !== null}
            title={loadBlocked ?? undefined}
            aria-label={resume ? "Continue from the resume point" : `Load ${label.toLowerCase()}`}
            className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}
          >
            {loadLabel}
          </button>
          <button
            type="button"
            onClick={onDelete}
            disabled={deleteDisabledReason !== null}
            title={deleteDisabledReason ?? undefined}
            aria-label={`Delete ${label.toLowerCase()}`}
            className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white`}
          >
            Delete
          </button>
        </div>
      </div>
    </article>
  );
}
