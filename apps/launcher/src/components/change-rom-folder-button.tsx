import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands, errorMessage, type LibraryFolder } from "../api/tauri-commands";
import { open } from "../backend";
import { useStartScan } from "../hooks/use-library-queries";
import { useErrorToast } from "./error-toast";

/** First-run "Change…": picks another ROM folder, adds it, then drops the previous
 *  ones (re-picking the same folder keeps it) and rescans. */
export function ChangeRomFolderButton({ current }: { current: LibraryFolder[] }) {
  const queryClient = useQueryClient();
  const startScan = useStartScan();
  const { showError } = useErrorToast();
  const [busy, setBusy] = useState(false);
  const change = async () => {
    const ref = await open({ directory: true, multiple: false, title: "Choose a ROM folder" });
    if (typeof ref !== "string") return;
    setBusy(true);
    try {
      const added = await commands.addLibraryFolder(ref);
      for (const folder of current) {
        if (folder.id !== added.id) await commands.removeLibraryFolder(folder.id);
      }
      startScan.mutate();
    } catch (err) {
      showError("Cannot change folder", errorMessage(err));
    } finally {
      setBusy(false);
      await queryClient.invalidateQueries();
    }
  };
  return (
    <button
      type="button"
      onClick={() => void change()}
      disabled={busy}
      className="h-9 w-fit cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
    >
      Change…
    </button>
  );
}
