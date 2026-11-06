import { useQuery, useQueryClient } from "@tanstack/react-query";
import { edition } from "../backend";
import { webCommands } from "../api/web-commands";

const KEY = ["web-folders-needing-access"] as const;

/** Web edition: folders whose access Chrome did not keep for this visit (unless
 *  "Allow on every visit" was chosen). One click re-asks for all of them. */
export function ReconnectFoldersBanner() {
  const queryClient = useQueryClient();
  const { data: folders = [] } = useQuery({
    queryKey: KEY,
    queryFn: webCommands.foldersNeedingAccess,
    enabled: edition === "web",
  });
  if (edition !== "web" || folders.length === 0) return null;
  const reconnect = async () => {
    await webCommands.reconnectFolders();
    await queryClient.invalidateQueries();
  };
  return (
    <div className="mb-6 flex flex-wrap items-center gap-3.5 rounded-card border border-border bg-surface px-[18px] py-1 text-sm text-text-soft">
      <span role="status">
        Reconnect {folders.length === 1 ? "folder" : `${folders.length} folders`}:{" "}
        <b className="text-text">{folders.join(", ")}</b> — the browser asks again on each visit unless you
        choose “Allow on every visit”.
      </span>
      <button
        type="button"
        onClick={() => void reconnect()}
        className="h-11 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-link"
      >
        Reconnect
      </button>
    </div>
  );
}
