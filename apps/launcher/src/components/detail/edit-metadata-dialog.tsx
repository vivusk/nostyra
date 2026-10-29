import { useEffect, useRef, useState, type ReactNode } from "react";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import { errorMessage } from "../../api/tauri-commands";
import {
  useEditMetadataForm,
  toUserEdit,
  DESCRIPTION_MAX,
  FIELD_MAX,
  PLAYERS_MAX,
  TITLE_MAX,
  YEAR_MAX,
  YEAR_MIN,
  type MetadataFormValues,
} from "../../hooks/use-edit-metadata-form";
import { useGenres, useResetGameMetadata, useUpdateGameMetadata } from "../../hooks/use-game-detail-queries";

const INPUT = "h-11 rounded-control border border-border-strong bg-surface-raised px-3 font-body text-[15px] text-text outline-none focus:border-accent";

interface EditMetadataDialogProps {
  game: GameDetail;
  onClose: () => void;
}

/** Native modal `<dialog>` (focus trap + Escape for free). Any save freezes the whole
 *  row against database refreshes until "Reset to database values". */
export function EditMetadataDialog({ game, onClose }: EditMetadataDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const { values, setField } = useEditMetadataForm(game);
  const [error, setError] = useState<string | null>(null);
  const update = useUpdateGameMetadata();
  const reset = useResetGameMetadata();
  const { data: genres = [] } = useGenres();

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  const save = () => {
    const { edit, error: invalid } = toUserEdit(game, values);
    if (!edit) return setError(invalid ?? "Invalid values.");
    setError(null);
    update.mutate({ gameId: game.id, edit }, { onSuccess: onClose, onError: (e) => setError(errorMessage(e)) });
  };
  const resetToDatabase = () => {
    if (!window.confirm("Reset this game's details to the database values? Your edits will be lost.")) return;
    reset.mutate(game.id, { onSuccess: onClose });
  };
  const field = (name: keyof MetadataFormValues) => ({
    value: values[name],
    onChange: (e: { target: { value: string } }) => setField(name, e.target.value),
  });

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="edit-metadata-title"
      className="m-auto w-[min(640px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <form
        method="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
        className="flex flex-col gap-4 p-7"
      >
        <h2 id="edit-metadata-title" className="m-0 font-display text-[22px] font-semibold">
          Edit metadata
        </h2>
        <Field label="Title">
          <input className={INPUT} maxLength={TITLE_MAX} {...field("title")} />
        </Field>
        <div className="grid grid-cols-2 gap-4">
          <Field label="Developer">
            <input className={INPUT} maxLength={FIELD_MAX} {...field("developer")} />
          </Field>
          <Field label="Publisher">
            <input className={INPUT} maxLength={FIELD_MAX} {...field("publisher")} />
          </Field>
          <Field label="Genre">
            <input className={INPUT} maxLength={FIELD_MAX} list="edit-metadata-genres" {...field("genre")} />
            <datalist id="edit-metadata-genres">
              {genres.map((genre) => (
                <option key={genre} value={genre} />
              ))}
            </datalist>
          </Field>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Year">
              <input className={INPUT} type="number" min={YEAR_MIN} max={YEAR_MAX} {...field("releaseYear")} />
            </Field>
            <Field label="Players">
              <input className={INPUT} type="number" min={1} max={PLAYERS_MAX} {...field("players")} />
            </Field>
          </div>
        </div>
        <Field label="Description">
          <textarea className={`${INPUT} h-32 resize-y py-2`} maxLength={DESCRIPTION_MAX} {...field("description")} />
        </Field>
        {error && (
          <p role="alert" className="m-0 text-sm text-warning">
            {error}
          </p>
        )}
        <p className="m-0 text-xs text-muted">Saved values are kept until you reset (rescans and database updates leave them alone).</p>
        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={resetToDatabase}
            disabled={!game.metadata?.userEdited || reset.isPending}
            className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
          >
            Reset to database values
          </button>
          <button type="button" onClick={onClose} className="ml-auto h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white">
            Cancel
          </button>
          <button type="submit" disabled={update.isPending} className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body font-bold text-white hover:bg-accent disabled:opacity-50">
            Save
          </button>
        </div>
      </form>
    </dialog>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5 text-sm text-text-soft">
      {label}
      {children}
    </label>
  );
}
