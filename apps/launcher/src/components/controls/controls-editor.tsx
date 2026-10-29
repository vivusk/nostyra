import { useId, useState, type ReactNode } from "react";
import type { BindingKey, GameplayConfigScope } from "../../api/tauri-commands-gameplay";
import type { InputDevice } from "../../api/tauri-commands-input";
import { ConfirmDialog } from "../confirm-dialog";
import { SegmentedControl } from "../gameplay/gameplay-setting-controls";
import { ControlsBindingRow } from "./controls-binding-row";
import { ControlsCaptureDialog } from "./controls-capture-dialog";
import { ControlsFixedRow } from "./controls-fixed-row";
import { gamepadLabel, systemActionLabel } from "./controls-labels";
import { useControlsEditor, type ControlsRow } from "./use-controls-editor";
import { useSystems } from "../../hooks/use-system-queries";

const DEVICE_CHOICES: { value: InputDevice; label: string }[] = [
  { value: "keyboard", label: "Keyboard" },
  { value: "gamepad", label: "Gamepad" },
];

/** Keyboard / gamepad bindings of one config layer: Settings → Controls (global) and
 *  Detail → Settings → Controls (this game; rows follow the defaults until overridden).
 *  Game buttons are shared by every system; only the buttons the shown system's pad has
 *  are listed (`systemId` for a game, a system picker for the global layer).
 *  Every capture goes through the backend `assign_binding` (steal + one write). */
export function ControlsEditor({ scope, systemId }: { scope: GameplayConfigScope; systemId?: string }) {
  const deviceId = useId();
  const systemPickerId = useId();
  const [device, setDevice] = useState<InputDevice>("keyboard");
  const { data: systems = [] } = useSystems();
  const [pickedSystem, setPickedSystem] = useState<string | null>(null);
  const [capturing, setCapturing] = useState<BindingKey | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const { state, isError } = useControlsEditor(scope, device);
  const gameScope = scope !== "global";

  if (!state) return isError ? <p className="m-0 text-sm text-text-soft">The controls could not be loaded.</p> : null;
  const { catalog, labelStyle, rows, warnings } = state;
  const shownSystemId = systemId ?? pickedSystem ?? systems[0]?.id;
  const shownSystem = systems.find((s) => s.id === shownSystemId);
  // The system's own button order (NES: B A Select Start D-pad); all rows until known.
  const actionRows = shownSystem
    ? shownSystem.padActions.flatMap((name) => rows.actions.filter((row) => row.key.endsWith(`.${name}`)))
    : rows.actions;
  const systemChoices = systems.map((s) => ({ value: s.id, label: s.shortLabel }));
  const addDisabledReason =
    device === "gamepad" && state.gameRunning ? "Close the game first: the gamepad is in use" : null;

  // Game buttons read as the shown system names them ("A (Y)" on the Mega Drive).
  const rowLabel = (key: BindingKey) => systemActionLabel(key, shownSystem);
  const renderRow = (row: ControlsRow) => (
    <ControlsBindingRow
      key={row.key}
      row={{ ...row, label: rowLabel(row.key) }}
      device={device}
      catalog={catalog}
      labelStyle={labelStyle}
      gameScope={gameScope}
      busy={state.busy}
      addDisabledReason={addDisabledReason}
      onAdd={() => setCapturing(row.key)}
      onSetBinds={(binds) => state.setBinds(row.key, binds)}
    />
  );
  const pad = (name: string) => gamepadLabel(name, catalog, labelStyle);

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center gap-4">
        <span id={`${deviceId}-label`} className="sr-only">
          Device
        </span>
        <SegmentedControl controlId={deviceId} choices={DEVICE_CHOICES} value={device} onChange={(v) => v && setDevice(v)} />
        <button
          type="button"
          onClick={() => setConfirmReset(true)}
          disabled={!state.hasOwnBindings}
          className="ml-auto h-10 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
        >
          Reset controls to defaults
        </button>
      </div>
      {warnings.length > 0 && (
        <div role="status" className="flex flex-col gap-1 rounded-control border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-text-soft">
          <b className="text-warning">Some bindings in your config files were adjusted (a shortcut keeps an input it shares with a game button):</b>
          {warnings.map((warning) => (
            <span key={warning}>{warning}</span>
          ))}
        </div>
      )}
      {device === "gamepad" && addDisabledReason && <p className="m-0 text-xs text-warning">{addDisabledReason}.</p>}

      {systemId === undefined && systemChoices.length > 1 && (
        <div className="flex flex-wrap items-center gap-3">
          <span id={`${systemPickerId}-label`} className="text-sm text-text-soft">
            Show buttons of
          </span>
          <SegmentedControl
            controlId={systemPickerId}
            choices={systemChoices}
            value={shownSystemId ?? null}
            onChange={(v) => v && setPickedSystem(v)}
          />
          <span className="text-xs text-muted">Buttons are shared by all systems.</span>
        </div>
      )}
      <ControlsGroup title="Game buttons">{actionRows.map(renderRow)}</ControlsGroup>
      <ControlsGroup title="Shortcuts">
        {rows.hotkeys.map(renderRow)}
        {device === "keyboard" ? (
          <ControlsFixedRow input="Esc" action="Pause menu" />
        ) : (
          <>
            <ControlsFixedRow input={pad("guide")} action="Pause menu" />
            <ControlsFixedRow input={`${pad("back")} + ${pad("start")} (hold)`} action="Pause menu" />
          </>
        )}
      </ControlsGroup>
      <p className="m-0 text-xs text-muted">
        Changes apply the next time a game starts. An input added here is removed from any other button or shortcut on the same
        device.
      </p>

      {capturing !== null && (
        <ControlsCaptureDialog
          device={device}
          target={capturing}
          title={rowLabel(capturing)}
          catalog={catalog}
          labelStyle={labelStyle}
          onCapture={(input) => state.assign(capturing, input)}
          onClose={() => setCapturing(null)}
        />
      )}
      {confirmReset && (
        <ConfirmDialog
          title={gameScope ? "Reset controls for this game?" : "Reset all controls to defaults?"}
          confirmLabel="Reset"
          onConfirm={state.resetAll}
          onClose={() => setConfirmReset(false)}
        >
          {gameScope
            ? "This game's own bindings are removed and it uses the default controls again. Its other settings stay."
            : "Every keyboard and gamepad binding and shortcut goes back to the built-in layout. Game-specific controls stay."}
        </ConfirmDialog>
      )}
    </div>
  );
}

/** Titled list of rows ("Game buttons", "Shortcuts"). */
function ControlsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <h3 className="m-0 text-xs font-bold tracking-[0.08em] text-muted uppercase">{title}</h3>
      <div className="flex flex-col">{children}</div>
    </div>
  );
}
