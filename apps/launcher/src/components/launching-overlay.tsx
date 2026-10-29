import { useGameSession } from "../hooks/use-game-session";
import { useGameDetail } from "../hooks/use-game-detail-queries";

/** "Launching…" overlay until the runner reports Started (≈0.7 s GPU init), then a
 *  slim "Now playing" bar while the game window is open. Rendered by the shell, so
 *  Play from any page shows it; the title comes from the (cached) game detail. */
export function LaunchingOverlay() {
  const { data: session } = useGameSession();
  const { data: detail } = useGameDetail(session?.gameId ?? null);
  if (!session) return null;
  const title = detail?.title ?? "game";

  if (session.phase === "launching") {
    return (
      <div role="status" className="fixed inset-0 z-40 flex items-center justify-center bg-app-bg/80 backdrop-blur-sm">
        <div className="flex items-center gap-4 rounded-card border border-border-strong bg-surface-raised px-7 py-5">
          <span aria-hidden="true" className="size-5 animate-spin rounded-full border-2 border-accent border-t-transparent" />
          <span className="font-display text-lg font-semibold">Launching {title}…</span>
        </div>
      </div>
    );
  }
  return (
    <div role="status" className="fixed bottom-6 left-1/2 z-30 -translate-x-1/2 rounded-full border border-border-strong bg-surface-raised px-5 py-2.5 text-sm text-text-soft shadow-xl">
      Now playing <b className="text-text">{title}</b> · close the game window to return
    </div>
  );
}
