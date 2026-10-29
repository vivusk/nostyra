// Choice lists and display labels for the v1 gameplay settings (Settings page and
// Detail → Settings tab). Values are the TOML values the runner reads.
import type { GameplayConfig, ResumeMode, VideoScaling } from "../../api/tauri-commands-gameplay";

/** One option of a select / segmented control; `null` = "Use default" (game scope). */
export interface Choice<V> {
  value: V | null;
  label: string;
}

const SHADER_NONE = "none";
/** Memory sizes offered for rewind; any other valid value (16–512) shows as "Custom". */
const REWIND_BUFFER_CHOICES = [32, 64, 128, 256];

/** Words shown upper-case in preset labels (kept in sync with the runner's `shader_label`). */
const SHADER_ACRONYMS: Record<string, string> = { crt: "CRT", lcd: "LCD" };

/** "crt-easymode" → "CRT Easymode", "zfast-lcd" → "Zfast LCD"; "none" → "None"; a `.slangp` path → "Custom: <path>". */
export function shaderLabel(shader: string, presets: string[]): string {
  if (shader === SHADER_NONE) return "None";
  if (!presets.includes(shader)) return `Custom: ${shader}`;
  return shader
    .split("-")
    .map((word) => SHADER_ACRONYMS[word] ?? word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

const SCALING_LABELS: Record<VideoScaling, string> = { aspect: "Aspect", integer: "Integer", stretch: "Stretch" };
const RESUME_MODE_LABELS: Record<ResumeMode, string> = { ask: "Ask", always: "Always", never: "Never" };
const runAheadLabel = (frames: number) => (frames === 0 ? "Off" : String(frames));
const bufferLabel = (mb: number) => (REWIND_BUFFER_CHOICES.includes(mb) ? `${mb} MB` : `Custom: ${mb} MB`);

/** Label of a key's value, as shown in options and in "Use default (…)". */
export function valueLabel<K extends keyof GameplayConfig>(key: K, value: GameplayConfig[K], presets: string[]): string {
  switch (key) {
    case "video.shader":
      return shaderLabel(value as string, presets);
    case "video.scaling":
      return SCALING_LABELS[value as VideoScaling];
    case "latency.run_ahead_frames":
      return runAheadLabel(value as number);
    case "rewind.enabled":
    case "resume.auto_save":
      return value ? "On" : "Off";
    case "resume.mode":
      return RESUME_MODE_LABELS[value as ResumeMode];
    case "rewind.buffer_mb":
    default:
      return bufferLabel(value as number);
  }
}

/** Fixed values offered for a key (before "Use default" and any read-only custom value). */
export function baseChoices(key: keyof GameplayConfig, presets: string[]): GameplayConfig[typeof key][] {
  switch (key) {
    case "video.shader":
      return [SHADER_NONE, ...presets];
    case "video.scaling":
      return ["aspect", "integer", "stretch"];
    case "latency.run_ahead_frames":
      return [0, 1, 2];
    case "rewind.enabled":
    case "resume.auto_save":
      return [true, false];
    case "resume.mode":
      return ["ask", "always", "never"];
    case "rewind.buffer_mb":
    default:
      return REWIND_BUFFER_CHOICES;
  }
}
