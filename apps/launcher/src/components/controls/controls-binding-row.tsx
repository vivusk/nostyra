import type { InputCatalog, InputDevice, LabelStyle } from "../../api/tauri-commands-input";
import { inputLabel } from "./controls-labels";
import type { ControlsRow } from "./use-controls-editor";

const SMALL_BUTTON =
  "h-8 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";

interface ControlsBindingRowProps {
  row: ControlsRow;
  device: InputDevice;
  catalog: InputCatalog;
  labelStyle: LabelStyle;
  /** Game scope: offers "Override" / "Use default". */
  gameScope: boolean;
  /** A binding write is in flight: edit buttons wait for the refetched row. */
  busy: boolean;
  /** Why "Add" is unavailable (pad capture while a game runs); null = enabled. */
  addDisabledReason: string | null;
  /** Opens the capture dialog for this row. */
  onAdd: () => void;
  /** Writes the row (`[]` = unbound) or `null` = follow the defaults again. */
  onSetBinds: (binds: string[] | null) => void;
}

/** One action / hotkey: label, up to two input chips (each removable), Add, Clear;
 *  in game scope an inherited row is muted with "Default" and an "Override" button. */
export function ControlsBindingRow({ row, device, catalog, labelStyle, gameScope, busy, addDisabledReason, onAdd, onSetBinds }: ControlsBindingRowProps) {
  const { label, binds, inherited } = row;
  const full = binds.length >= catalog.maxBinds;
  const addReason = addDisabledReason ?? (full ? `Up to ${catalog.maxBinds} inputs; remove one first` : null);

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-border py-2.5 first:border-t-0 first:pt-0">
      <span className="flex min-w-36 flex-1 items-center gap-2 text-sm font-semibold text-text">
        {label}
        {gameScope && !inherited && <span className="rounded-md bg-[#2A2A35] px-2 py-0.5 text-[11px] font-bold text-[#DAD5FF]">Overridden</span>}
      </span>
      <div className="flex flex-wrap items-center gap-2">
        {binds.length === 0 && <span className="text-sm text-muted">{inherited ? "Default: unbound" : "Unbound"}</span>}
        {binds.map((input) => {
          const text = inputLabel(device, input, catalog, labelStyle);
          return inherited ? (
            <span key={input} className="rounded-md border border-border px-2.5 py-1 text-sm text-muted">
              {text}
            </span>
          ) : (
            <span key={input} className="inline-flex items-center gap-1 rounded-md bg-surface-raised py-0.5 pr-0.5 pl-2.5 text-sm font-semibold text-text">
              {text}
              <button
                type="button"
                aria-label={`Remove ${text} from ${label}`}
                onClick={() => onSetBinds(binds.filter((b) => b !== input))}
                disabled={busy}
                className="size-7 cursor-pointer rounded-md border-0 bg-transparent text-muted hover:text-white disabled:cursor-default disabled:opacity-50"
              >
                ✕
              </button>
            </span>
          );
        })}
        {inherited ? (
          <>
            <span className="text-xs text-muted">Default</span>
            <button type="button" onClick={() => onSetBinds(binds)} disabled={busy} aria-label={`Override ${label} for this game`} className={SMALL_BUTTON}>
              Override
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              onClick={onAdd}
              disabled={busy || addReason !== null}
              title={addReason ?? undefined}
              aria-label={`Add an input to ${label}`}
              className={SMALL_BUTTON}
            >
              Add
            </button>
            <button
              type="button"
              onClick={() => onSetBinds([])}
              disabled={busy || binds.length === 0}
              aria-label={`Clear ${label}`}
              className={SMALL_BUTTON}
            >
              Clear
            </button>
            {gameScope && (
              <button type="button" onClick={() => onSetBinds(null)} disabled={busy} aria-label={`Use the default for ${label}`} className={SMALL_BUTTON}>
                Use default
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
