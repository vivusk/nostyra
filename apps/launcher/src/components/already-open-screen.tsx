/** Web edition, second tab: Nostyra Online already runs in another tab of this
 *  browser (it holds the library database and the saves); nothing is opened here. */
export function AlreadyOpenScreen() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 p-10 text-center font-body">
      <h1 className="m-0 font-display text-2xl font-bold text-text">Nostyra Online is already open</h1>
      <p className="m-0 max-w-md text-sm text-text-soft">
        It is running in another tab of this browser. Switch to that tab, or close it and reload this one.
      </p>
    </div>
  );
}
