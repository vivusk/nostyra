import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  DATA_FOLDER_MOVE_PROGRESS,
  dataFolderCommands,
  type DataFolderCheck,
  type DataFolderMoveProgress,
} from "../../api/tauri-commands-data-folder";
import { errorMessage } from "../../api/tauri-commands";

interface DataFolderChangeDialogProps {
  path: string;
  check: DataFolderCheck;
  onClose: () => void;
}

/** "Change data folder?": move the current data (and overwrite files already there)
 *  or start the new folder as it is; then the app restarts. Locked while moving. */
export function DataFolderChangeDialog({ path, check, onClose }: DataFolderChangeDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const [moveData, setMoveData] = useState(check.files > 0);
  const [overwrite, setOverwrite] = useState(false);
  const [working, setWorking] = useState(false);
  const [progress, setProgress] = useState<DataFolderMoveProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    dialogRef.current?.showModal();
    cancelRef.current?.focus();
    const subscription = listen<DataFolderMoveProgress>(DATA_FOLDER_MOVE_PROGRESS, (e) => setProgress(e.payload));
    return () => void subscription.then((unlisten) => unlisten());
  }, []);

  const confirm = async () => {
    setWorking(true);
    setError(null);
    try {
      await dataFolderCommands.changeDataFolder(path, moveData, overwrite);
      // Success restarts the app; reaching here means nothing happened.
    } catch (err) {
      setError(errorMessage(err));
      setWorking(false);
      setProgress(null);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      // Escape must not close it mid-move.
      onCancel={(e) => {
        if (working) e.preventDefault();
      }}
      onClose={onClose}
      aria-labelledby="data-folder-dialog-title"
      className="m-auto w-[min(480px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="data-folder-dialog-title" className="m-0 font-display text-[22px] font-semibold">
          Change data folder?
        </h2>
        <p className="m-0 rounded-control bg-surface-raised px-3.5 py-2.5 text-sm break-all text-text-soft">{path}</p>

        {check.files > 0 && (
          <label className="flex cursor-pointer items-start gap-3 text-sm">
            <input
              type="checkbox"
              checked={moveData}
              disabled={working}
              onChange={(e) => setMoveData(e.target.checked)}
              className="mt-0.5 size-4 accent-accent"
            />
            <span>
              Move my current data there
              <span className="block text-xs text-muted">
                {check.files} files: library, cores, saves, save states, settings. Unchecked: Nostyra starts with what the new folder has.
              </span>
            </span>
          </label>
        )}
        {moveData && check.conflicts > 0 && (
          <label className="flex cursor-pointer items-start gap-3 text-sm">
            <input
              type="checkbox"
              checked={overwrite}
              disabled={working}
              onChange={(e) => setOverwrite(e.target.checked)}
              className="mt-0.5 size-4 accent-accent"
            />
            <span>
              Overwrite {check.conflicts} {check.conflicts === 1 ? "file" : "files"} that already exist there
              <span className="block text-xs text-muted">Unchecked: those files are kept, and their old copies stay in the current folder.</span>
            </span>
          </label>
        )}

        <p className="m-0 text-xs text-muted">Nostyra restarts to use the new folder.</p>
        {working && (
          <p role="status" className="m-0 text-sm text-text-soft">
            {moveData && progress ? `Moving data… ${progress.done} / ${progress.total} files` : moveData ? "Moving data…" : "Restarting…"}
          </p>
        )}
        {error && (
          <p role="alert" className="m-0 text-sm text-danger">
            {error}
          </p>
        )}

        <div className="flex justify-end gap-3">
          <button
            ref={cancelRef}
            type="button"
            onClick={onClose}
            disabled={working}
            className="h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void confirm()}
            disabled={working}
            className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
          >
            {moveData ? "Move and restart" : "Switch and restart"}
          </button>
        </div>
      </div>
    </dialog>
  );
}
