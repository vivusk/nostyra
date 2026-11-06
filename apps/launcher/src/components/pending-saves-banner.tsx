import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { edition, listen } from "../backend";
import { WEB_EVENTS, webCommands } from "../api/web-commands";

const KEY = ["web-pending-saves"] as const;

/** Web edition: SRAM a session could not write to the Data folder (the file was
 *  busy, or access was lost) is kept in the browser; this banner says so and writes
 *  it again on Retry (the launcher also retries at every start). */
export function PendingSavesBanner() {
  const queryClient = useQueryClient();
  const { data: paths = [] } = useQuery({ queryKey: KEY, queryFn: webCommands.pendingSaves, enabled: edition === "web" });
  const retry = useMutation({
    mutationFn: webCommands.retryPendingSaves,
    onSettled: () => void queryClient.invalidateQueries({ queryKey: KEY }),
  });
  useEffect(() => {
    if (edition !== "web") return;
    const unlisten = listen(WEB_EVENTS.pendingSaves, () => void queryClient.invalidateQueries({ queryKey: KEY }));
    return () => void unlisten.then((stop) => stop());
  }, [queryClient]);
  if (edition !== "web" || paths.length === 0) return null;
  return (
    <div className="mb-6 flex flex-wrap items-center gap-3.5 rounded-card border border-warning/60 bg-surface px-[18px] py-1 text-sm text-text-soft">
      <span role="status">
        Save could not be written to the Data folder — it is kept in this browser
        ({paths.length === 1 ? "1 file" : `${paths.length} files`}).
      </span>
      <button
        type="button"
        onClick={() => retry.mutate()}
        disabled={retry.isPending}
        className="h-11 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-link disabled:cursor-default disabled:opacity-60"
      >
        Retry
      </button>
    </div>
  );
}
