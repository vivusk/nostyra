import { useGameplayConfig, useSetGameplayConfigValue } from "../../hooks/use-gameplay-config-queries";
import { GameplayConfigForm } from "../gameplay/gameplay-config-form";

/** Settings → Gameplay defaults: the global layer (config/global.toml), used by every
 *  game that has no value of its own. Saves on change. */
export function GameplayDefaultsSection() {
  const { data: view, isError } = useGameplayConfig(null);
  const setValue = useSetGameplayConfigValue();

  if (!view) return isError ? <p className="m-0 text-sm text-text-soft">Gameplay settings could not be loaded.</p> : null;
  return (
    <>
      <p className="m-0 text-xs text-muted">
        Used by every game without its own setting (Game → Settings). Changes apply next time you play.
      </p>
      <GameplayConfigForm
        layer={view.global}
        fallback={view.resolved}
        presets={view.presets}
        onChange={(key, value) => setValue.mutate({ scope: "global", key, value })}
      />
    </>
  );
}
