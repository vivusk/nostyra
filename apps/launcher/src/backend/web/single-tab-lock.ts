// One Nostyra Online tab per browser profile: the OPFS database (opfs-sahpool)
// holds exclusive file handles and two tabs would race on the same saves. The
// first tab holds the Web Lock `nostyra-online` for its lifetime; later tabs see
// it taken and show "already open in another tab" without opening anything.

const LOCK_NAME = "nostyra-online";

/** `true` when this tab now holds the lock (kept until the tab closes). */
export function holdSingleTabLock(): Promise<boolean> {
  if (!navigator.locks) return Promise.resolve(true);
  return new Promise((resolve) => {
    void navigator.locks.request(LOCK_NAME, { ifAvailable: true }, (lock) => {
      if (!lock) {
        resolve(false);
        return undefined;
      }
      resolve(true);
      // Never settles: the lock is released when the tab goes away.
      return new Promise<never>(() => undefined);
    });
  });
}
