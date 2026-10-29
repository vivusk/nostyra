import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { errorMessage } from "../../api/tauri-commands";
import { dataFolderCommands, type DataFolderCheck } from "../../api/tauri-commands-data-folder";
import { useSetupState } from "../../hooks/use-library-queries";
import { useErrorToast } from "../error-toast";
import { DataFolderChangeDialog } from "./data-folder-change-dialog";

/** Settings → Data folder: the current path and Change… (pick → check → confirm dialog). */
export function SettingsDataFolderSection() {
  const { data: setup } = useSetupState();
  const { showError } = useErrorToast();
  const [picked, setPicked] = useState<{ path: string; check: DataFolderCheck } | null>(null);
  // Anything using the data folder blocks the change (the backend checks again).
  const busy = Boolean(setup?.activeGameId != null || setup?.scanning || setup?.artworkFetching || setup?.coreDownloading);

  const pick = async () => {
    const path = await open({ directory: true, multiple: false, title: "Choose a data folder" });
    if (typeof path !== "string") return;
    try {
      setPicked({ path, check: await dataFolderCommands.checkDataFolder(path) });
    } catch (err) {
      showError("Cannot use that folder", errorMessage(err));
    }
  };

  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <p className="m-0 min-w-0 flex-1 text-sm break-all text-text-soft">{setup?.dataRoot}</p>
        <button
          type="button"
          onClick={() => void pick()}
          disabled={busy}
          className="h-10 shrink-0 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
        >
          Change…
        </button>
      </div>
      <p className="m-0 text-xs text-muted">
        Library database, cores, saves and save states live here.
        {busy && " Wait for the running game, scan or download to finish to change it."}
      </p>
      {picked && <DataFolderChangeDialog path={picked.path} check={picked.check} onClose={() => setPicked(null)} />}
    </>
  );
}
