import type { Game } from "../../api/tauri-commands";
import { assetUrl } from "../../lib/asset-url";
import { formatPlayTime } from "../../lib/game-display";
import { relativeTime } from "../../lib/relative-time";
import { CoverImage } from "../cover-image";

interface ContinuePlayingRowProps {
  games: Game[];
  onOpen: (game: Game) => void;
}

/** Up to 4 recently played games: snap (pixel art, cover-fit), region badge, title,
 *  "2 hours ago · 41h 12m". No progress bar: there is no progress data. */
export function ContinuePlayingRow({ games, onOpen }: ContinuePlayingRowProps) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-5">
      {games.map((game) => (
        <button
          key={game.id}
          type="button"
          onClick={() => onOpen(game)}
          className="flex cursor-pointer flex-col overflow-hidden rounded-[14px] border-0 bg-[#17171C] p-0 text-left font-body text-text transition-transform hover:-translate-y-[3px]"
        >
          <div className="relative">
            <CoverImage
              src={assetUrl(game.heroPath)}
              title={game.title}
              fit="cover"
              className="h-[150px] w-full [image-rendering:pixelated]"
              monogramClassName="text-[44px]"
            />
            {game.region && (
              <span className="absolute bottom-2.5 left-3 rounded-md bg-black/60 px-2 py-1 text-xs text-[#E4E4EA]">
                {game.region}
              </span>
            )}
          </div>
          <div className="flex flex-col gap-2 px-4 pt-3.5 pb-4">
            <span className="truncate font-bold">{game.title}</span>
            <span className="text-[13px] text-muted">
              {game.lastPlayedAt !== null && `${relativeTime(game.lastPlayedAt)} · `}
              {formatPlayTime(game.totalPlayS)}
            </span>
          </div>
        </button>
      ))}
    </div>
  );
}
