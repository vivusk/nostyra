// Typed wrappers for the gameplay commands: layered config (global / per game),
// save-state slots and the resume point
// (src-tauri/src/commands/{gameplay_config,save_state,resume}_commands.rs).
// Loading a slot = `commands.playGame(gameId, slot)`; Continue = `playGame(gameId, "auto")`.
import { invoke } from "@tauri-apps/api/core";

export type VideoScaling = "aspect" | "integer" | "stretch";
/** What Play does when a resume point exists. */
export type ResumeMode = "ask" | "always" | "never";

/**
 * Gameplay settings, keyed by the dotted TOML key (the same key
 * `setGameplayConfigValue` takes), so each UI field maps to one key.
 * Controls (binding keys) live in `GameplayConfigView.input`, never here.
 */
export interface GameplayConfig {
  /** "none", a bundled preset name (`presets`) or an absolute `.slangp` path. */
  "video.shader": string;
  "video.scaling": VideoScaling;
  /** 0 (off) – 2. */
  "latency.run_ahead_frames": number;
  "rewind.enabled": boolean;
  /** 16 – 512 MiB. */
  "rewind.buffer_mb": number;
  /** Launcher: Continue / Start fresh dialog, direct Continue, or always fresh. */
  "resume.mode": ResumeMode;
  /** Runner writes the resume point on every clean exit. */
  "resume.auto_save": boolean;
  /** RetroAchievements (global layer only; Settings → RetroAchievements). */
  "achievements.enabled": boolean;
  /** Ignored unless the build shows the hardcore toggle. */
  "achievements.hardcore": boolean;
  "achievements.sound": boolean;
  "achievements.leaderboard_popups": boolean;
  /** Unknown soft-patched game: use the original ROM's achievements (softcore). */
  "achievements.patched_rom_fallback": boolean;
}

export type GameplayConfigKey = keyof GameplayConfig;

/** A binding key: `"input_keyboard.b"`, `"hotkeys_gamepad.save_state"`, … */
export type BindingKey = string;
/** Flat binding keys → input names (`[]` = unbound). */
export type BindingMap = Record<BindingKey, string[]>;

/** The Controls editor's layers (same layering as the gameplay keys). */
export interface InputConfigView {
  /** Every binding key with the inputs the runner will use (conflicts resolved). */
  resolved: BindingMap;
  /** Keys this game overrides; missing = "Default". Empty without a game. */
  game: Partial<BindingMap>;
  /** Keys set in global.toml. */
  global: Partial<BindingMap>;
  /** Hand-edited conflicts the resolver fixed (hotkey wins), for a notice. */
  warnings: string[];
}

export interface GameplayConfigView {
  /** What the runner will use (all layers over the built-in defaults). */
  resolved: GameplayConfig;
  /** Keys this game overrides; a missing key = "Use default". Empty without a game. */
  game: Partial<GameplayConfig>;
  /** Keys set in global.toml (launcher Settings defaults). */
  global: Partial<GameplayConfig>;
  /** Bundled shader presets besides "none". */
  presets: string[];
  /** Controls (binding sections). */
  input: InputConfigView;
}

/** Layer a write goes to. */
export type GameplayConfigScope = "global" | { gameId: number };

/** A numbered save slot 0–9 or the resume point. */
export type StateSlotId = number | "auto";

export interface SaveStateSlot {
  /** 0–9, or "auto" (the resume point, listed first). */
  slot: StateSlotId;
  /** Unix seconds; also the thumbnail cache-buster (`assetUrl(path, createdAt)`). */
  createdAt: number;
  /** Null when the slot's sidecar is missing. */
  coreName: string | null;
  coreVersion: string | null;
  /** Absolute slotN.png / auto.png path for `assetUrl`; null when no thumbnail was written. */
  thumbnailPath: string | null;
  sizeBytes: number;
  /** Why the runner would refuse this state ("Made by another Dolphin version"); null = loadable. */
  incompatibleReason: string | null;
}

/** `state-incompatible`: the runner would refuse the resume point (Start fresh only). */
export type ResumeWarning = "sram-newer" | "patch-changed" | "core-version-changed" | "state-incompatible";

/** What Play needs to choose between plain launch, Continue and the Resume dialog. */
export interface ResumeInfo {
  mode: ResumeMode;
  /** A resume point (auto.state + sidecar) exists. */
  available: boolean;
  /** Unix seconds; null when not available. */
  createdAt: number | null;
  /** Absolute auto.png path for `assetUrl`. */
  thumbnailPath: string | null;
  warnings: ResumeWarning[];
  /** With `state-incompatible`: why the resume point cannot be loaded. */
  incompatibleReason: string | null;
  /** Next launch runs in achievement hardcore: no resume, no state loading. */
  hardcore: boolean;
}

export const gameplayCommands = {
  /** Effective + layer values for a game, or for the global defaults (no id). */
  getGameplayConfig: (gameId?: number) =>
    invoke<GameplayConfigView>("get_gameplay_config", { gameId: gameId ?? null }),
  /** Sets one key, or clears it with `null` ("Use default"). Rejects with kind "invalid". */
  setGameplayConfigValue: <K extends GameplayConfigKey>(
    scope: GameplayConfigScope,
    key: K,
    value: GameplayConfig[K] | null,
  ) => invoke<void>("set_gameplay_config_value", { scope, key, value }),
  listSaveStates: (gameId: number) => invoke<SaveStateSlot[]>("list_save_states", { gameId }),
  /** Removes the slot's state, sidecar and thumbnail. Rejects with kind "busy" while the game runs. */
  deleteSaveState: (gameId: number, slot: StateSlotId) => invoke<void>("delete_save_state", { gameId, slot }),
  getResumeInfo: (gameId: number) => invoke<ResumeInfo>("get_resume_info", { gameId }),
};
