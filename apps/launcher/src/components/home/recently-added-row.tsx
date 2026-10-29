import type { Game } from "../../api/tauri-commands";
import { assetUrl } from "../../lib/asset-url";
import { CoverImage } from "../cover-image";

interface RecentlyAddedRowProps {
  games: Game[];
  onOpen: (game: Game) => void;
}

/** "1994 · Platformer", falling back to the region when metadata is missing. */
function addedMeta(game: Game): string {
  const parts = [game.releaseYear, game.genre].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : (game.region ?? "");
}

/** Up to 6 newest games: 3:4 cover tile (letterboxed art) + title + year/genre. */
export function RecentlyAddedRow({ games, onOpen }: RecentlyAddedRowProps) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-5">
      {games.map((game) => (
        <button
          key={game.id}
          type="button"
          onClick={() => onOpen(game)}
          className="flex cursor-pointer flex-col gap-2.5 border-0 bg-transparent p-0 text-left font-body text-text transition-transform hover:-translate-y-[3px]"
        >
          <CoverImage
            src={assetUrl(game.coverPath, game.coverCheckedAt)}
            title={game.title}
            className="aspect-[3/4] w-full rounded-card border border-white/6"
          />
          <span className="truncate text-sm font-semibold">{game.title}</span>
          <span className="-mt-1.5 text-xs text-muted">{addedMeta(game)}</span>
        </button>
      ))}
    </div>
  );
}
