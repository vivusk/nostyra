import { useEffect, useRef, type ReactNode } from "react";

interface ConfirmDialogProps {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  /** Red confirm button: the action deletes data for good. */
  danger?: boolean;
  onConfirm: () => void;
  /** Cancel, Escape, or after confirming. */
  onClose: () => void;
}

/** Native modal `<dialog>` in the Edit metadata style (focus trap + Escape for free);
 *  focus starts on Cancel so Enter/A never destroys anything by accident. */
export function ConfirmDialog({ title, children, confirmLabel, danger = false, onConfirm, onClose }: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    dialogRef.current?.showModal();
    cancelRef.current?.focus();
  }, []);

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="confirm-dialog-title"
      className="m-auto w-[min(440px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="confirm-dialog-title" className="m-0 font-display text-[22px] font-semibold">
          {title}
        </h2>
        <p className="m-0 leading-relaxed text-text-soft">{children}</p>
        <div className="flex justify-end gap-3">
          <button
            ref={cancelRef}
            type="button"
            onClick={onClose}
            className="h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => {
              onConfirm();
              onClose();
            }}
            className={`h-11 cursor-pointer rounded-control border-0 px-5 font-body font-bold text-white ${
              danger ? "bg-danger-strong hover:bg-danger-hover" : "bg-accent-strong hover:bg-accent"
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </dialog>
  );
}
