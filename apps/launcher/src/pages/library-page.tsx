import { useCallback, useDeferredValue, useEffect, useRef, useState } from "react";
import type { Game } from "../api/tauri-commands";
import { ArtworkStatusBanner } from "../components/artwork-status-banner";
import { ConfirmDialog } from "../components/confirm-dialog";
import { LibraryFilterPills } from "../components/library-filter-pills";
import { LibraryGenreSelect } from "../components/library-genre-select";
import { LibrarySystemFilter } from "../components/library-system-filter";
import { LibrarySortSelect } from "../components/library-sort-select";
import { LibraryToolbar } from "../components/library-toolbar";
import { ScanStatusBanner } from "../components/scan-status-banner";
import { VirtualGameGrid } from "../components/virtual-game-grid";
import { useGameSession } from "../hooks/use-game-session";
import { usePlayGame } from "../hooks/use-play-game";
import {
  useGames,
  useLibrarySummary,
  useRemoveMissingGames,
  useScanProgress,
  useSetupState,
  useStartScan,
} from "../hooks/use-library-queries";
import { supportedSystemsText, useLibrarySystems, useSystems } from "../hooks/use-system-queries";
import { MISSING_REMOVAL_WARNING } from "../lib/missing-game-removal-text";
import { useNavigation } from "../navigation/navigation-context";
import type { LibraryViewState } from "../navigation/route-types";
import { PageHeader } from "./page-header";

/** Library per mockup: counts, search, sort, filters, genre, banners, virtual grid.
 *  Controls live in the navigation store (the grid restores/saves the scroll offset),
 *  so Back from Detail returns to the same place. */
export function LibraryPage({ focusSearch = false }: { focusSearch?: boolean }) {
  const { library, updateLibrary, navigate, scrollRef } = useNavigation();
  const { search, sort, filter, genre, system } = library;
  const deferredSearch = useDeferredValue(search);
  const { data: librarySystems = [] } = useLibrarySystems();
  // A remembered system filter whose games are all gone would hide everything.
  const platform = librarySystems.length > 1 && librarySystems.some((s) => s.id === system) ? system : undefined;
  const { data: games = [], isPending } = useGames({
    search: deferredSearch,
    sort,
    filter,
    genre: genre || undefined,
    platform,
  });
  const { data: summary } = useLibrarySummary();
  const { data: setup } = useSetupState();
  const { data: scan } = useScanProgress();
  const { data: session } = useGameSession();
  const play = usePlayGame();
  const startScan = useStartScan();
  const removeMissing = useRemoveMissingGames();
  const [confirmRemoveAll, setConfirmRemoveAll] = useState(false);
  const headerRef = useRef<HTMLDivElement>(null);

  // Stable callbacks keep the memoised cards from re-rendering on every scroll update.
  const onPlay = useCallback((game: Game) => play(game.id), [play]);
  // The focus hand-off from Home is one-shot: drop the flag so Back from Detail
  // does not focus the search box again.
  useEffect(() => {
    if (focusSearch) navigate({ page: "library" });
  }, [focusSearch, navigate]);

  // A new search/sort/filter shows a different list: start it from the top.
  const changeView = (patch: Partial<LibraryViewState>) => {
    updateLibrary(patch);
    scrollRef.current?.scrollTo({ top: 0 });
  };
  const onOpen = useCallback((game: Game) => navigate({ page: "detail", gameId: game.id }), [navigate]);

  const subtitle = summary
    ? `${summary.total} games · ${summary.matched} matched with No-Intro · ${summary.needsReview} need review` +
      (summary.missing > 0 ? ` · ${summary.missing} missing` : "")
    : "Loading…";
  // Bulk removal is offered on the Missing list only; it removes every missing game
  // (search/genre do not narrow it — the confirm names the count).
  const missingCount = summary?.missing ?? 0;
  const showRemoveAll = filter === "missing" && missingCount > 0 && !scan;

  return (
    <div className="flex flex-col gap-6">
      <div ref={headerRef} className="flex flex-col gap-6">
        {/* Container query, not viewport: the sidebar eats into the window. Narrow = title
            above a full-width search row; wide = title left, search + Rescan right. */}
        <div className="@container">
          <div className="flex flex-col gap-4 @3xl:flex-row @3xl:items-end">
            <PageHeader title="Library" subtitle={subtitle} />
            <LibraryToolbar
              autoFocusSearch={focusSearch}
              search={search}
              onSearchChange={(value) => changeView({ search: value })}
              gameCount={summary?.total ?? 0}
              scanning={Boolean(scan) || startScan.isPending}
              onRescan={() => startScan.mutate()}
            />
          </div>
        </div>
        {/* One-tap chips first (most used); the dropdown row ends with Sort right above the
            grid it orders. Both rows run left to right, so a narrow window wraps them as
            compact pills instead of stranding one on the right. */}
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2.5">
            <LibraryFilterPills value={filter} onChange={(value) => changeView({ filter: value })} />
            {showRemoveAll && (
              <button
                type="button"
                onClick={() => setConfirmRemoveAll(true)}
                disabled={removeMissing.isPending}
                className="h-10 cursor-pointer rounded-[20px] border border-danger/50 bg-transparent px-4 font-body text-sm font-medium text-danger hover:border-danger hover:bg-danger/10 disabled:cursor-default disabled:opacity-50"
              >
                {removeMissing.isPending ? "Removing…" : `Remove all ${missingCount} missing`}
              </button>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2.5">
            <LibrarySystemFilter value={platform ?? ""} onChange={(value) => changeView({ system: value })} />
            <LibraryGenreSelect value={genre} onChange={(value) => changeView({ genre: value })} />
            <LibrarySortSelect value={sort} onChange={(value) => changeView({ sort: value })} />
          </div>
        </div>
        {setup?.datMissing && (
          <p role="status" className="m-0 rounded-card border border-warning/40 bg-warning/10 px-[18px] py-3 text-sm text-warning">
            Game database missing — all games need review.
          </p>
        )}
        <ScanStatusBanner />
        <ArtworkStatusBanner />
      </div>
      {games.length > 0 ? (
        <VirtualGameGrid
          games={games}
          showSystem={librarySystems.length > 1}
          sessionBusy={Boolean(session)}
          onOpen={onOpen}
          onPlay={onPlay}
          headerRef={headerRef}
        />
      ) : isPending ? null : (
        <EmptyLibrary filtered={summary !== undefined && summary.total > 0} scanning={Boolean(scan)} />
      )}
      {confirmRemoveAll && (
        <ConfirmDialog
          title={`Remove ${missingCount} missing ${missingCount === 1 ? "game" : "games"}?`}
          confirmLabel="Remove"
          danger
          onConfirm={() => removeMissing.mutate(null)}
          onClose={() => setConfirmRemoveAll(false)}
        >
          {MISSING_REMOVAL_WARNING}
        </ConfirmDialog>
      )}
    </div>
  );
}

function EmptyLibrary({ filtered, scanning }: { filtered: boolean; scanning: boolean }) {
  const { data: systems } = useSystems();
  const text = filtered
    ? "No games match this search or filter."
    : scanning
      ? "Looking for games…"
      : `No ${supportedSystemsText(systems)} games found in your folders. Add another folder in Settings.`;
  return (
    <section className="rounded-card border border-border bg-surface px-6 py-10 text-center text-text-soft">{text}</section>
  );
}
