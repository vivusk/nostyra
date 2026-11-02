import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../api/query-keys";
import { commands, errorMessage } from "../../api/tauri-commands";
import { useErrorToast } from "../error-toast";

/** Settings → ROM folders: opt into scanning one-ROM ZIP archives. */
export function SettingsZipScan() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  const setting = useQuery({ queryKey: queryKeys.scanZipArchives, queryFn: commands.getScanZipArchives });
  const update = useMutation({
    mutationFn: commands.setScanZipArchives,
    onSuccess: (_, enabled) => {
      queryClient.setQueryData(queryKeys.scanZipArchives, enabled);
      void queryClient.invalidateQueries({ queryKey: queryKeys.scanProgress });
    },
    onError: (error) => showError("Cannot change ZIP scanning", errorMessage(error)),
  });

  return (
    <div className="flex flex-col gap-2">
      <label className="flex cursor-pointer items-center gap-3 text-sm text-text-soft">
        <input
          type="checkbox"
          checked={setting.data ?? false}
          disabled={setting.isPending || setting.isError || update.isPending}
          onChange={(event) => update.mutate(event.target.checked)}
          className="h-4 w-4 accent-[var(--color-accent)] disabled:cursor-default disabled:opacity-50"
        />
        Scan ZIP archives
      </label>
      <p className="m-0 text-xs text-muted">
        One ROM per ZIP: NES, SNES, Mega Drive / Genesis, Master System, Game Gear, SG-1000 or GBA.
        Games are extracted temporarily when played. Changing this option rescans the library.
        Turning it off keeps previously scanned ZIP games playable.
      </p>
      {setting.isError && <p role="alert" className="m-0 text-xs text-muted">Cannot read ZIP scan settings: {errorMessage(setting.error)}</p>}
    </div>
  );
}
