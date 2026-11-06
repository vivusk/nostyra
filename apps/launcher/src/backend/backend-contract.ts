// The one seam between the React launcher and what runs it. Chosen at build time
// (vite alias `@nostyra/backend`): `backend/tauri` (desktop: Rust commands, Tauri
// events, native dialogs, asset protocol) or `backend/web` (nostyra.com/play:
// commands implemented in the browser over the File System Access API and wasm).
// The `api/*` wrappers and the few components that used Tauri directly go through
// this contract only, so the web bundle never imports `@tauri-apps/*`.

/** What the running edition can do; the UI hides or disables the rest. */
export interface Capabilities {
  /** Download libretro cores (desktop: buildbot; web: cores ship with the site). */
  coreDownload: boolean;
  achievements: boolean;
  /** Shader presets, run-ahead and rewind settings rows. */
  shaders: boolean;
  /** Scaling row (aspect / integer / stretch); the web player has one fit mode. */
  videoScaling: boolean;
  runAhead: boolean;
  rewind: boolean;
  /** Log files + bug report bundle. */
  logs: boolean;
  /** Reveal a ROM / open a folder in the OS file manager. */
  revealInFolder: boolean;
  /** PlayStation memory card import / second card. */
  memoryCards: boolean;
  /** Gamepad capture through the native SDL3 backend (web: Gamepad API instead). */
  gamepadCaptureNative: boolean;
  /** Move the whole data folder elsewhere (web: choose a folder instead). */
  dataFolderMove: boolean;
}

/** Which edition this build is. */
export type Edition = "desktop" | "web";

/** A backend event, shaped like Tauri's (`event.payload`). */
export interface BackendEvent<T> {
  payload: T;
}

export type Unlisten = () => void;

/** Native-dialog options the launcher uses (subset of Tauri's plugin-dialog `open`). */
export interface OpenDialogOptions {
  title?: string;
  directory?: boolean;
  multiple?: boolean;
  defaultPath?: string;
  filters?: { name: string; extensions: string[] }[];
}

/** `busy`: the edition refuses to run here (web: already open in another tab). */
export type BootResult = "ready" | "busy";

export interface Backend {
  edition: Edition;
  /** Runs once before the UI renders. */
  boot(): Promise<BootResult>;
  capabilities: Capabilities;
  /** Runs a command by its Rust name (`list_games`) with camelCase arguments. */
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  /** Subscribes to a backend event; resolves to the unsubscribe function. */
  listen<T>(event: string, handler: (event: BackendEvent<T>) => void): Promise<Unlisten>;
  /** Folder / file picker. Returns opaque refs the commands accept (desktop: paths). */
  open(options: OpenDialogOptions): Promise<string | string[] | null>;
  /** Image URL of a backend asset ref (cover, hero, state thumbnail…), usable in
   *  render. May be a token that `resolveAsset` turns into a real URL. */
  assetUrl(ref: string, version?: number | null): string;
  /** Real URL of an `assetUrl` result (identity for URLs that are already real). */
  resolveAsset(src: string): Promise<string | null>;
}

/** Message of the error a command rejects with when the edition lacks it. */
export const DESKTOP_ONLY_MESSAGE = "Available in the desktop app only";
