// Artwork commands of the web backend (src-tauri artwork_commands.rs): start /
// cancel the job, progress, cache stats / clear; plus the auto-fetch after a scan
// (artwork_auto_fetch_hook.rs). The auto-fetch setting itself is in settings/.
import { clearArtwork } from "../db/artwork-queries";
import { artworkAutoFetch } from "../db/settings-queries";
import { onScanFinished } from "../scan/scan-command";
import { commandError, type CommandTable } from "../web-command-table";
import { forgetResolvedAssets } from "./asset-resolver";
import { cancelArtworkFetch, currentArtworkProgress, isArtworkFetching, startArtworkFetch } from "./artwork-fetch-job";
import { artworkCacheStats, clearArtworkFiles } from "./artwork-store";

// New games get art without the user asking (setting on unless "0").
onScanFinished((cancelled) => {
  if (cancelled) return;
  void artworkAutoFetch().then(
    (enabled) => enabled && startArtworkFetch(null),
    (error: unknown) => console.warn("cannot read the artwork auto-fetch setting", error),
  );
});

export const artworkCommands: CommandTable = {
  start_artwork_fetch: async ({ gameId }) => startArtworkFetch(typeof gameId === "number" ? gameId : null),
  cancel_artwork_fetch: async () => cancelArtworkFetch(),
  get_artwork_progress: async () => currentArtworkProgress(),
  get_artwork_cache_stats: () => artworkCacheStats(),
  /** Refused while a job runs: a worker could record a file that was just deleted. */
  clear_artwork_cache: async () => {
    if (isArtworkFetching()) throw commandError("artwork", "Wait for the artwork fetch to finish (or cancel it).");
    await clearArtworkFiles();
    await clearArtwork();
    forgetResolvedAssets();
  },
};
