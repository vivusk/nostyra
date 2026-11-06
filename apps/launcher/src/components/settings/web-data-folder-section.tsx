import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "../../api/tauri-commands";
import { webCommands, type WebDataFolderSwitch } from "../../api/web-commands";
import { useErrorToast } from "../error-toast";

const BUTTON =
  "h-10 shrink-0 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";
const STATUS_KEY = ["web-data-folder"] as const;

/** Settings → Data folder (web): browser storage, or a folder shared with the
 *  desktop app (saves, config, BIOS). Choosing a folder copies the browser's files
 *  into it without overwriting any. */
export function WebDataFolderSection() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  const { data: status } = useQuery({ queryKey: STATUS_KEY, queryFn: webCommands.dataFolderStatus });
  const [result, setResult] = useState<WebDataFolderSwitch | null>(null);
  const [busy, setBusy] = useState(false);

  const act = async (action: () => Promise<unknown>) => {
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
  const choose = () => act(async () => setResult(await webCommands.chooseDataFolder()));

  const label =
    status?.kind === "folder"
      ? `${status.name}${status.sharedWithDesktop ? " — shared with Nostyra desktop" : ""}`
      : "Browser storage (this browser only)";
  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <p className="m-0 min-w-0 flex-1 text-sm break-all text-text-soft">{label}</p>
        <button type="button" onClick={() => void choose()} disabled={busy} className={BUTTON}>
          Choose folder…
        </button>
        {status?.kind === "folder" && (
          <>
            <button type="button" onClick={() => void act(webCommands.useBrowserStorage)} disabled={busy} className={BUTTON}>
              Use browser storage
            </button>
            <button type="button" onClick={() => void act(webCommands.clearBrowserCopy)} disabled={busy} className={BUTTON}>
              Clear browser copy
            </button>
          </>
        )}
      </div>
      <p className="m-0 text-xs text-muted">
        Saves, settings, controls and BIOS files live here. To share them with the desktop app, choose the
        desktop Data folder. Chrome cannot open folders inside AppData ("contains system files"): in the
        desktop app use Settings → Data folder → Change… to move it, e.g. to Documents\Nostyra, then choose
        that folder here. Do not play the same game in both apps at the same time.
      </p>
      {status?.needsAccess && (
        <p className="m-0 text-xs text-warning">Access to this folder was not granted this visit: reconnect it.</p>
      )}
      {result && (
        <p className="m-0 text-xs text-text-soft">
          Copied {result.copied} file{result.copied === 1 ? "" : "s"} from browser storage.
          {result.conflicts.length > 0 &&
            ` Kept ${result.conflicts.length} existing file${result.conflicts.length === 1 ? "" : "s"} in the folder (not overwritten): ${result.conflicts.slice(0, 5).join(", ")}${result.conflicts.length > 5 ? ", …" : ""}. Their browser copies stay until you clear them.`}
        </p>
      )}
    </>
  );
}
