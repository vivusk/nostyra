import { useId } from "react";
import type { GameplayConfig, GameplayConfigKey } from "../../api/tauri-commands-gameplay";
import { baseChoices, valueLabel, type Choice } from "./gameplay-config-choices";
import { SegmentedControl, SelectControl, SettingRow } from "./gameplay-setting-controls";

type GameplayValue = GameplayConfig[GameplayConfigKey];

interface GameplayConfigFormProps {
  /** Values set in the layer being edited (global.toml or this game's file). */
  layer: Partial<GameplayConfig>;
  /** What applies where `layer` has no value. */
  fallback: GameplayConfig;
  /** Game scope: values a "Use default (…)" choice inherits; omitted for the global layer. */
  inherited?: GameplayConfig;
  /** Bundled shader presets besides "none". */
  presets: string[];
  /** `null` = "Use default" (clears the key from the layer). */
  onChange: (key: GameplayConfigKey, value: GameplayValue | null) => void;
}

const ROWS: { key: GameplayConfigKey; label: string; help?: string; control: "select" | "segmented" | "toggle" }[] = [
  { key: "video.shader", label: "Shader", control: "select" },
  { key: "video.scaling", label: "Scaling", control: "segmented" },
  { key: "latency.run_ahead_frames", label: "Run-ahead", help: "Reduces input lag; uses more CPU", control: "segmented" },
  { key: "rewind.enabled", label: "Rewind", help: "Hold the Rewind shortcut in game (Backspace or LT / L2 by default)", control: "toggle" },
  { key: "rewind.buffer_mb", label: "Rewind memory", control: "select" },
  { key: "resume.mode", label: "Resume", help: "On Play: ask to continue, always continue, or always start fresh", control: "segmented" },
  { key: "resume.auto_save", label: "Save resume point on exit", help: "Remembers where you quit, for Continue", control: "toggle" },
];

/** The gameplay settings (video, latency, rewind, resume) for one config layer, shared by Settings → Gameplay
 *  defaults (global) and Detail → Settings (game, with "Use default" choices and
 *  "Overridden" badges). Each change is written at once by the caller. */
export function GameplayConfigForm({ layer, fallback, inherited, presets, onChange }: GameplayConfigFormProps) {
  const idPrefix = useId();
  return (
    <div className="flex flex-col">
      {ROWS.map(({ key, label, help, control }) => {
        const controlId = `${idPrefix}-${key.replace(".", "-")}`;
        const own = layer[key];
        // Game scope shows "Use default" (null) when the layer leaves the key unset.
        const current: GameplayValue | null = own ?? (inherited ? null : fallback[key]);
        const change = (value: GameplayValue | null) => {
          if (value !== current) onChange(key, value);
        };
        const choices = buildChoices(key, current, inherited?.[key], presets);
        return (
          <SettingRow key={key} controlId={controlId} label={label} help={help} overridden={Boolean(inherited) && own !== undefined}>
            {control === "select" ? (
              <SelectControl controlId={controlId} choices={choices} value={current} onChange={change} />
            ) : control === "toggle" && !inherited ? (
              <input
                id={controlId}
                type="checkbox"
                checked={Boolean(current)}
                onChange={(e) => change(e.target.checked)}
                className="h-5 w-5 cursor-pointer accent-[var(--color-accent)]"
              />
            ) : (
              <SegmentedControl controlId={controlId} choices={choices} value={current} onChange={change} />
            )}
          </SettingRow>
        );
      })}
    </div>
  );
}

/** "Use default (…)" first (game scope), the fixed values, then a value set in the file
 *  outside that list (custom shader path, odd rewind size) as a read-only extra. */
function buildChoices(
  key: GameplayConfigKey,
  current: GameplayValue | null,
  inherited: GameplayValue | undefined,
  presets: string[],
): Choice<GameplayValue>[] {
  const values: GameplayValue[] = [...baseChoices(key, presets)];
  if (current !== null && !values.includes(current)) values.push(current);
  const choices: Choice<GameplayValue>[] = values.map((value) => ({ value, label: valueLabel(key, value, presets) }));
  if (inherited !== undefined) choices.unshift({ value: null, label: `Use default (${valueLabel(key, inherited, presets)})` });
  return choices;
}
