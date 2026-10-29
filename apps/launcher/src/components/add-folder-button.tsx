import { open } from "@tauri-apps/plugin-dialog";
import { useAddLibraryFolder } from "../hooks/use-library-queries";

/** Opens the OS folder picker and adds the folder (then a rescan starts). */
export function AddFolderButton({ label = "Add ROM folder" }: { label?: string }) {
  const addFolder = useAddLibraryFolder();
  const pick = async () => {
    const path = await open({ directory: true, multiple: false, title: "Choose a ROM folder" });
    if (typeof path === "string") addFolder.mutate(path);
  };
  return (
    <button
      type="button"
      onClick={() => void pick()}
      disabled={addFolder.isPending}
      className="h-11 w-fit cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
    >
      {label}
    </button>
  );
}
