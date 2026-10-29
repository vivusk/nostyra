import { memo } from "react";
import type { Game } from "../api/tauri-commands";
import { assetUrl } from "../lib/asset-url";
import { gameMetaLine } from "../lib/game-display";
import { CoverImage } from "./cover-image";

interface GameCoverCardProps {
  game: Game;
  /** System badge ("NES"); omitted while the library holds a single system. */
  systemLabel?: string;
  /** A game is running or launching: Play is disabled everywhere. */
  sessionBusy: boolean;
  onOpen: (game: Game) => void;
  onPlay: (game: Game) => void;
}

/** Library card: cover (or monogram) tile opens Detail; Play shows on hover/focus.
 *  Two real buttons so keyboard/gamepad focus reaches both. */
export const GameCoverCard = memo(function GameCoverCard({
  game,
  systemLabel,
  sessionBusy,
  onOpen,
  onPlay,
}: GameCoverCardProps) {
  const unavailable = game.missing || sessionBusy;
  return (
    <div role="group" aria-label={game.title} className="group/card flex flex-col gap-2.5">
      <div className="relative">
        <button
          type="button"
          onClick={() => onOpen(game)}
          aria-label={`Open ${game.title}`}
          title={game.romPath}
          className="block w-full cursor-pointer rounded-card border-0 bg-transparent p-0 hover:outline-2 hover:outline-offset-2 hover:outline-accent"
        >
          <CoverImage
            src={assetUrl(game.coverPath, game.coverCheckedAt)}
            title={game.title}
            className={`aspect-[3/4] w-full rounded-card border border-white/5 ${game.missing ? "opacity-40" : ""}`}
          />
        </button>
        <div className="pointer-events-none absolute top-2.5 left-2.5 flex flex-wrap gap-1.5">
          {systemLabel && (
            <span className="rounded-md bg-black/60 px-2 py-1 text-[11px] font-bold text-[#DAD5FF]">{systemLabel}</span>
          )}
          {game.datName === null && (
            <span className="rounded-md bg-warning px-2 py-1 text-[11px] font-bold text-[#1A1406]">NEEDS REVIEW</span>
          )}
        </div>
        {game.missing && (
          <span className="pointer-events-none absolute top-2.5 right-2.5 rounded-md bg-black/60 px-2 py-1 text-[11px] font-bold">
            MISSING
          </span>
        )}
        {!unavailable && (
          <button
            type="button"
            onClick={() => onPlay(game)}
            aria-label={`Play ${game.title}`}
            className="absolute inset-x-2.5 bottom-2.5 flex h-10 cursor-pointer items-center justify-center gap-2 rounded-lg border-0 bg-accent-strong font-body text-sm font-bold text-white opacity-0 transition-opacity group-hover/card:opacity-100 focus-visible:opacity-100"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M7 4v16l13-8z" />
            </svg>
            Play
          </button>
        )}
      </div>
      <span className="line-clamp-2 h-10 text-sm leading-5 font-semibold">{game.title}</span>
      <span className="-mt-1.5 h-[18px] truncate text-xs leading-[18px] text-muted">
        {gameMetaLine(game.region, game.totalPlayS, game.lastPlayedAt)}
      </span>
    </div>
  );
});
