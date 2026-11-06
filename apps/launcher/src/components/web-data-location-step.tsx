import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "../api/tauri-commands";
import { webCommands } from "../api/web-commands";
import { useErrorToast } from "./error-toast";

const CARD = "flex flex-1 basis-56 flex-col gap-3 rounded-card border border-border-strong bg-app-bg p-4";
const PRIMARY =
  "h-10 cursor-pointer self-start rounded-control border-0 bg-accent-strong px-4 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50";
const SECONDARY =
  "h-10 cursor-pointer self-start rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";

/** Web first run, step 2: where saves, settings, controls and BIOS files live —
 *  browser storage or a folder on this PC (ideally the desktop app's Data folder).
 *  Either answer ends the setup; Settings → Data folder changes it later. */
export function WebDataLocationStep() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  const [busy, setBusy] = useState(false);
  const answer = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
    } catch (err) {
      showError("Data folder", errorMessage(err));
    } finally {
      setBusy(false);
      await queryClient.invalidateQueries();
    }
  };
  return (
    <div className="flex flex-col gap-3">
      <p className="m-0 text-sm text-text-soft">
        Saves, settings, controls and BIOS files are kept here. You can change it later in Settings → Data folder.
      </p>
      <div className="flex flex-wrap gap-3">
        <section className={CARD} aria-label="Folder on this PC">
          <h3 className="m-0 font-display text-base font-semibold">A folder on this PC</h3>
          <ul className="m-0 flex flex-col gap-1 pl-4 text-xs text-text-soft">
            <li>Shared with the Nostyra desktop app: choose its Data folder to keep the same saves and controls.</li>
            <li>Survives clearing browser data; easy to back up.</li>
            <li className="text-muted">Chrome / Edge ask for access again on later visits unless you allow it on every visit.</li>
            <li className="text-muted">Folders inside AppData cannot be opened by the browser (e.g. use Documents\Nostyra).</li>
          </ul>
          <button type="button" disabled={busy} onClick={() => void answer(webCommands.chooseDataFolder)} className={PRIMARY}>
            Choose folder…
          </button>
        </section>
        <section className={CARD} aria-label="Browser storage">
          <h3 className="m-0 font-display text-base font-semibold">Browser storage</h3>
          <ul className="m-0 flex flex-col gap-1 pl-4 text-xs text-text-soft">
            <li>Nothing to set up; never asks for permission.</li>
            <li className="text-muted">Only in this browser profile: not shared with the desktop app or other browsers.</li>
            <li className="text-muted">Lost if you clear this site's data; harder to back up.</li>
          </ul>
          <button type="button" disabled={busy} onClick={() => void answer(webCommands.useBrowserStorage)} className={SECONDARY}>
            Use browser storage
          </button>
        </section>
      </div>
    </div>
  );
}
