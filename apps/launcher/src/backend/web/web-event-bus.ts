// In-page replacement for Tauri events: web command modules `emit`, the launcher's
// `listen` subscriptions receive `{ payload }` exactly like desktop events.
import type { BackendEvent, Unlisten } from "../backend-contract";

type Handler = (event: BackendEvent<unknown>) => void;

const handlers = new Map<string, Set<Handler>>();

export function emit<T>(event: string, payload: T): void {
  for (const handler of handlers.get(event) ?? []) {
    try {
      handler({ payload });
    } catch (error) {
      console.error(`event ${event} handler failed`, error);
    }
  }
}

export async function listen<T>(
  event: string,
  handler: (event: BackendEvent<T>) => void,
): Promise<Unlisten> {
  const set = handlers.get(event) ?? new Set<Handler>();
  handlers.set(event, set);
  const wrapped = handler as Handler;
  set.add(wrapped);
  return () => {
    set.delete(wrapped);
  };
}
