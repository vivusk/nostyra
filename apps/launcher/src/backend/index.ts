// What the launcher imports instead of `@tauri-apps/*`. `@nostyra/backend` is
// aliased at build time (vite.config.ts / tsconfig*.json) to `backend/tauri` or
// `backend/web`; see `backend-contract.ts`.
import active from "@nostyra/backend";
import type { BackendEvent, OpenDialogOptions, Unlisten } from "./backend-contract";

export type { BackendEvent, BootResult, Capabilities, Edition, OpenDialogOptions, Unlisten } from "./backend-contract";
export { DESKTOP_ONLY_MESSAGE } from "./backend-contract";

/** Which edition this build is. */
export const edition = active.edition;
/** What this edition can do (UI gating). */
export const capabilities = active.capabilities;

/** Runs before the UI renders (web: single-tab lock). */
export function boot() {
  return active.boot();
}

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return active.invoke<T>(command, args);
}

export function listen<T>(event: string, handler: (event: BackendEvent<T>) => void): Promise<Unlisten> {
  return active.listen<T>(event, handler);
}

export function open(options: OpenDialogOptions): Promise<string | string[] | null> {
  return active.open(options);
}

/** Render-time image URL of a backend asset ref; `null` for no ref. */
export function assetUrl(ref: string | null, version?: number | null): string | null {
  return ref ? active.assetUrl(ref, version) : null;
}

export function resolveAsset(src: string): Promise<string | null> {
  return active.resolveAsset(src);
}
