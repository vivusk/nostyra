import { useCallback, useMemo, useState } from "react";
import type { Game, GameSort } from "../api/tauri-commands";
import { AddFolderButton } from "../components/add-folder-button";
import { ContinuePlayingRow } from "../components/home/continue-playing-row";
import { HomeHeroBanner } from "../components/home/home-hero-banner";
import { HomeHeroSelector } from "../components/home/home-hero-selector";
import { HomeSection } from "../components/home/home-section";
import { HomeTopBar } from "../components/home/home-top-bar";
import { RecentlyAddedRow } from "../components/home/recently-added-row";
import { useGameSession } from "../hooks/use-game-session";
import { usePlayGame } from "../hooks/use-play-game";
import { useHomeData } from "../hooks/use-home-queries";
import { useStartScan } from "../hooks/use-library-queries";
import { supportedSystemsText, useSystems } from "../hooks/use-system-queries";
import { buildHeroEntries } from "../lib/home-hero-entries";
import { useNavigation } from "../navigation/navigation-context";

/** Home per mockup: top bar, hero + selector, Continue playing, Recently added. */
export function HomePage() {
  const { navigate, updateLibrary } = useNavigation();
  const { data, isPending, isError } = useHomeData();
  const { data: session } = useGameSession();
  const play = usePlayGame();
  const entries = useMemo(() => (data ? buildHeroEntries(data) : []), [data]);
  // Selection follows the game, so a data refresh that reorders entries keeps it.
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected = Math.max(0, entries.findIndex((entry) => entry.game.id === selectedId));

  const onPlay = useCallback((game: Game) => play(game.id), [play]);
  const onOpen = useCallback((game: Game) => navigate({ page: "detail", gameId: game.id }), [navigate]);
  const seeAll = (sort: GameSort) => {
    updateLibrary({ sort, filter: "all", genre: "", search: "", scrollOffset: 0 });
    navigate({ page: "library" });
  };

  if (isPending) return null;
  if (isError) {
    return <p className="m-0 text-text-soft">Home could not be loaded. Open the Library from the sidebar.</p>;
  }
  const hero = entries[selected];
  return (
    <div className="flex flex-col gap-10">
      <HomeTopBar />
      {hero ? (
        <section aria-label="Featured" className="flex flex-wrap gap-5">
          <HomeHeroBanner
            game={hero.game}
            reason={hero.reason}
            sessionBusy={Boolean(session)}
            onPlay={onPlay}
            onOpen={onOpen}
          />
          <HomeHeroSelector entries={entries} selected={selected} onSelect={(i) => setSelectedId(entries[i].game.id)} />
        </section>
      ) : (
        <EmptyHome />
      )}
      {data && data.continuePlaying.length > 0 && (
        <HomeSection id="home-continue" title="Continue playing" onSeeAll={() => seeAll("recently-played")}>
          <ContinuePlayingRow games={data.continuePlaying} onOpen={onOpen} />
        </HomeSection>
      )}
      {data && data.recentlyAdded.length > 0 && (
        <HomeSection id="home-added" title="Recently added" onSeeAll={() => seeAll("recently-added")}>
          <RecentlyAddedRow games={data.recentlyAdded} onOpen={onOpen} />
        </HomeSection>
      )}
    </div>
  );
}

/** No games yet: one call to action (add a folder or rescan). */
function EmptyHome() {
  const startScan = useStartScan();
  const { data: systems } = useSystems();
  return (
    <section className="flex flex-col items-start gap-4 rounded-[18px] border border-border bg-surface px-10 py-12">
      <h1 className="m-0 font-display text-3xl font-bold">Your library is empty</h1>
      <p className="m-0 text-text-soft">Add a folder with {supportedSystemsText(systems)} games, or rescan your folders.</p>
      <div className="flex flex-wrap gap-3">
        <AddFolderButton label="Add games" />
        <button
          type="button"
          onClick={() => startScan.mutate()}
          className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white"
        >
          Rescan
        </button>
      </div>
    </section>
  );
}
