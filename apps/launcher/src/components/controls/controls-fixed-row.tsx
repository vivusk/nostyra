/** A read-only shortcut row ("Esc — Pause menu (fixed)"): inputs users can never
 *  rebind, listed so the Shortcuts group shows every way into the pause menu. */
export function ControlsFixedRow({ input, action }: { input: string; action: string }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-border py-2.5">
      <span className="min-w-36 flex-1 text-sm font-semibold text-muted">{action}</span>
      <span className="rounded-md border border-border px-2.5 py-1 text-sm text-muted">{input}</span>
      <span className="text-xs text-muted">Fixed</span>
    </div>
  );
}
