// Web commands are plain async functions keyed by the Rust command name, so the
// `api/*` wrappers stay identical on both editions. Each group module (library,
// data folder, play, gameplay…) exports a table; `index.ts` merges them.
import { DESKTOP_ONLY_MESSAGE } from "../backend-contract";

export type CommandArgs = Record<string, unknown>;
export type CommandHandler = (args: CommandArgs) => Promise<unknown>;
export type CommandTable = Record<string, CommandHandler>;

/** Rejects like a Rust `CommandError` (`{ kind, message }`). */
export function commandError(kind: string, message: string): { kind: string; message: string } {
  return { kind, message };
}

/** Handler for a feature the web edition does not have. */
export const desktopOnly: CommandHandler = async () => {
  throw commandError("unsupported", DESKTOP_ONLY_MESSAGE);
};

/** Handler that always returns `value` (stubs and no-op settings). */
export function constant<T>(value: T): CommandHandler {
  return async () => value;
}

/** Merges group tables; a command defined twice is a programming error. */
export function mergeTables(...tables: CommandTable[]): CommandTable {
  const merged: CommandTable = {};
  for (const table of tables) {
    for (const [name, handler] of Object.entries(table)) {
      if (name in merged) throw new Error(`web command ${name} defined twice`);
      merged[name] = handler;
    }
  }
  return merged;
}
