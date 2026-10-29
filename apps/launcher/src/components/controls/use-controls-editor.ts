// State + writes of the Controls editor for one layer (global defaults or one game):
// rows per device (game buttons, shortcuts), inherited vs override, and the mutations.
// Steal rules live in the backend (`assign_binding`); this hook only reports them.
import type { BindingKey, GameplayConfigScope } from "../../api/tauri-commands-gameplay";
import type { InputCatalog, InputDevice, LabelStyle } from "../../api/tauri-commands-input";
import { useGameSession } from "../../hooks/use-game-session";
import { useAssignBinding, useGameplayConfig, useResetControls, useSetBindingValue } from "../../hooks/use-gameplay-config-queries";
import { useGamepadLabelStyle, useInputCatalog } from "../../hooks/use-input-catalog-query";
import { useErrorToast } from "../error-toast";
import { bindingKeyLabel, deviceSections, inputLabel } from "./controls-labels";

/** One action / hotkey row. */
export interface ControlsRow {
  key: BindingKey;
  label: string;
  /** Inputs the runner will use for this row (conflicts resolved). */
  binds: string[];
  /** Game scope only: the game has no value, the row follows the defaults. */
  inherited: boolean;
}

export interface ControlsEditorState {
  catalog: InputCatalog;
  labelStyle: LabelStyle;
  /** Game buttons and shortcuts of `device`, in catalog order. */
  rows: { actions: ControlsRow[]; hotkeys: ControlsRow[] };
  /** Conflicts the backend fixed in hand-edited files (hotkey wins). */
  warnings: string[];
  /** The edited layer holds at least one binding key ("Reset controls" enabled). */
  hasOwnBindings: boolean;
  /** A binding write is in flight: rows must not edit from a stale `binds` snapshot. */
  busy: boolean;
  /** Pad capture is refused while a game runs. */
  gameRunning: boolean;
  /** Binds `input` to `key` (steal) and toasts "Z removed from Y" for what it took. */
  assign: (key: BindingKey, input: string) => void;
  /** Writes the row's inputs (`[]` = unbound) or `null` = use the defaults. */
  setBinds: (key: BindingKey, binds: string[] | null) => void;
  resetAll: () => void;
}

/** `null` while the config or catalog is still loading; `isError` when either failed. */
export function useControlsEditor(scope: GameplayConfigScope, device: InputDevice) {
  const gameId = scope === "global" ? null : scope.gameId;
  const config = useGameplayConfig(gameId);
  const catalog = useInputCatalog();
  const { data: labelStyle = "xbox" } = useGamepadLabelStyle();
  const { data: session } = useGameSession();
  const assignBinding = useAssignBinding();
  const setBindingValue = useSetBindingValue();
  const resetControls = useResetControls();
  const { showNotice } = useErrorToast();

  const isError = config.isError || catalog.isError;
  const view = config.data?.input;
  if (!view || !catalog.data) return { state: null, isError };
  const cat = catalog.data;
  const layer = gameId === null ? view.global : view.game;
  const sections = deviceSections(device);

  const toRow = (section: string) => (name: string): ControlsRow => {
    const key = `${section}.${name}`;
    return {
      key,
      label: bindingKeyLabel(key),
      binds: view.resolved[key] ?? [],
      inherited: gameId !== null && layer[key] === undefined,
    };
  };

  const state: ControlsEditorState = {
    catalog: cat,
    labelStyle,
    rows: { actions: cat.padActions.map(toRow(sections.actions)), hotkeys: cat.hotkeys.map(toRow(sections.hotkeys)) },
    warnings: view.warnings,
    hasOwnBindings: Object.keys(layer).length > 0,
    busy: assignBinding.isPending || setBindingValue.isPending || resetControls.isPending,
    gameRunning: Boolean(session),
    assign: (key, input) =>
      assignBinding.mutate(
        { scope, device, target: key, input },
        {
          onSuccess: ({ removedFrom }) => {
            if (removedFrom.length === 0) return;
            const from = removedFrom.map(bindingKeyLabel).join(", ");
            showNotice(`${inputLabel(device, input, cat, labelStyle)} removed from ${from}`);
          },
        },
      ),
    setBinds: (key, binds) => setBindingValue.mutate({ scope, key, value: binds }),
    resetAll: () => resetControls.mutate(scope),
  };
  return { state, isError };
}
