import { useState } from "react";
import type { GameplayConfig, GameplayConfigKey } from "../../api/tauri-commands-gameplay";
import { useGameplayConfig, useSetGameplayConfigValue } from "../../hooks/use-gameplay-config-queries";
import { ConfirmDialog } from "../confirm-dialog";
import { ControlsEditor } from "../controls/controls-editor";
import { GameplayConfigForm } from "../gameplay/gameplay-config-form";

/** Detail → Settings: this game's layer (game/<system>/<rom> [<ID>].toml). Each row offers
 *  "Use default (<value>)"; "Reset all to defaults" clears every gameplay override
 *  (asks first) and leaves controls alone: they live in `view.input` and have their own
 *  "Reset controls to defaults" in the Controls group below. */
export function GameSettingsTab({ gameId, platformId }: { gameId: number; platformId: string }) {
  const { data: view, isError } = useGameplayConfig(gameId);
  const { data: defaults } = useGameplayConfig(null);
  const setValue = useSetGameplayConfigValue();
  const [confirmReset, setConfirmReset] = useState(false);

  if (!view) return isError ? <p className="m-0 text-text-soft">The settings for this game could not be loaded.</p> : null;

  const keys = Object.keys(view.resolved) as GameplayConfigKey[];
  const overriddenKeys = keys.filter((key) => view.game[key] !== undefined);
  // An unset key already resolves to the inherited value (system/core layers included);
  // for an overridden key the global defaults view is the closest stand-in.
  const inherited = Object.fromEntries(
    keys.map((key) => [key, view.game[key] === undefined ? view.resolved[key] : (defaults?.resolved[key] ?? view.resolved[key])]),
  ) as unknown as GameplayConfig;
  const scope = { gameId };
  const resetAll = () => overriddenKeys.forEach((key) => setValue.mutate({ scope, key, value: null }));

  return (
    <>
      <section aria-labelledby="game-settings-title" className="flex max-w-[720px] flex-col gap-5 rounded-[14px] border border-border bg-surface p-7">
        <div className="flex flex-wrap items-start gap-4">
          <div className="flex flex-1 flex-col gap-1">
            <h2 id="game-settings-title" className="m-0 font-display text-[22px] font-semibold">
              Game settings
            </h2>
            <p className="m-0 text-sm text-muted">Changes apply next time you play.</p>
          </div>
          <button
            type="button"
            onClick={() => setConfirmReset(true)}
            disabled={overriddenKeys.length === 0}
            className="h-10 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
          >
            Reset all to defaults
          </button>
        </div>
        <GameplayConfigForm
          layer={view.game}
          fallback={view.resolved}
          inherited={inherited}
          presets={view.presets}
          onChange={(key, value) => setValue.mutate({ scope, key, value })}
        />
        {confirmReset && (
          <ConfirmDialog
            title="Reset all settings for this game?"
            confirmLabel="Reset"
            onConfirm={resetAll}
            onClose={() => setConfirmReset(false)}
          >
            This game's own values are removed and it uses the default settings again. Its controls stay.
          </ConfirmDialog>
        )}
      </section>
      <section aria-labelledby="game-controls-title" className="flex max-w-[720px] flex-col gap-5 rounded-[14px] border border-border bg-surface p-7">
        <div className="flex flex-col gap-1">
          <h2 id="game-controls-title" className="m-0 font-display text-[22px] font-semibold">
            Controls
          </h2>
          <p className="m-0 text-sm text-muted">Each button follows the default controls (Settings → Controls) until you override it.</p>
        </div>
        <ControlsEditor scope={scope} systemId={platformId} />
      </section>
    </>
  );
}
