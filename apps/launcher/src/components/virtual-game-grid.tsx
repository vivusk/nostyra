import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Game } from "../api/tauri-commands";
import { useNavigation } from "../navigation/navigation-context";
import { systemLabel, useSystems } from "../hooks/use-system-queries";
import { GameCoverCard } from "./game-cover-card";

/** Grid geometry (mockup: 170 px min card, 22 px column gap, 28 px row gap). */
const MIN_CARD = 170;
const COLUMN_GAP = 22;
const ROW_GAP = 28;
/** Below the 3:4 tile: gap 10 + two title lines 40 + gap 4 + meta 18 (fixed heights). */
const CARD_TEXT_HEIGHT = 72;

interface VirtualGameGridProps {
  games: Game[];
  /** Badge each card with its system (library holds games of several systems). */
  showSystem: boolean;
  sessionBusy: boolean;
  onOpen: (game: Game) => void;
  onPlay: (game: Game) => void;
  /** Everything above the grid inside the scroll area (its height feeds scrollMargin). */
  headerRef: RefObject<HTMLElement | null>;
}

/** Row-virtualised card grid: only visible rows (+ overscan) are mounted, so 1 000+
 *  games scroll at 60 fps. The scroll element is the shell's `<main>`; the header
 *  above the grid can change height (toolbar wrap, banners), so its size is observed
 *  and fed to `scrollMargin`. */
export function VirtualGameGrid({ games, showSystem, sessionBusy, onOpen, onPlay, headerRef }: VirtualGameGridProps) {
  const { data: systems } = useSystems();
  const { scrollRef, library, updateLibrary } = useNavigation();
  const gridRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [scrollMargin, setScrollMargin] = useState(0);

  useLayoutEffect(() => {
    const grid = gridRef.current;
    const scroller = scrollRef.current;
    if (!grid || !scroller) return;
    const measure = () => {
      setWidth(grid.clientWidth);
      const offset = grid.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
      setScrollMargin(Math.round(offset));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    if (headerRef.current) observer.observe(headerRef.current);
    return () => observer.disconnect();
  }, [scrollRef, headerRef]);

  // Restore the scroll offset once the grid has real geometry (the rows exist), and
  // save it when the grid unmounts. Lives here, not in the page, so an evicted or
  // still-loading games list can neither lose the offset nor overwrite it with 0.
  const restored = useRef(false);
  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    if (restored.current || width === 0 || !scroller) return;
    restored.current = true;
    scroller.scrollTop = library.scrollOffset;
  }, [width, scrollRef, library.scrollOffset]);
  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    return () => updateLibrary({ scrollOffset: scroller.scrollTop });
    // Unmount only: saved once on leave.
  }, []);

  const columns = Math.max(1, Math.floor((width + COLUMN_GAP) / (MIN_CARD + COLUMN_GAP)));
  const cardWidth = width > 0 ? (width - COLUMN_GAP * (columns - 1)) / columns : MIN_CARD;
  const rowHeight = Math.round((cardWidth * 4) / 3 + CARD_TEXT_HEIGHT + ROW_GAP);
  const rowCount = Math.ceil(games.length / columns);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    // ~8 cards each side regardless of column count (keeps mounted cards near 60).
    overscan: Math.ceil(8 / columns),
    scrollMargin,
  });
  useLayoutEffect(() => virtualizer.measure(), [virtualizer, rowHeight, scrollMargin]);

  return (
    <div ref={gridRef} className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((row) => (
        <div
          key={row.key}
          className="absolute inset-x-0 top-0 grid"
          style={{
            transform: `translateY(${row.start - scrollMargin}px)`,
            gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
            columnGap: COLUMN_GAP,
          }}
        >
          {games.slice(row.index * columns, row.index * columns + columns).map((game) => (
            <GameCoverCard
              key={game.id}
              game={game}
              systemLabel={showSystem ? systemLabel(systems, game.platformId) : undefined}
              sessionBusy={sessionBusy}
              onOpen={onOpen}
              onPlay={onPlay}
            />
          ))}
        </div>
      ))}
    </div>
  );
}
