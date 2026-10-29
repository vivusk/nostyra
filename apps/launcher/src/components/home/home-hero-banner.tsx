import type { Game } from "../../api/tauri-commands";
import { assetUrl } from "../../lib/asset-url";
import { useResumeInfo } from "../../hooks/use-resume-info-query";
import { systemLabel, useSystems } from "../../hooks/use-system-queries";
import { formatPlayTime, homeMetaLine, monogram, tileColor } from "../../lib/game-display";
import { heroEyebrow, type HeroReason } from "../../lib/home-hero-entries";
import { relativeTimeInSentence } from "../../lib/relative-time";

interface HomeHeroBannerProps {
  game: Game;
  reason: HeroReason;
  sessionBusy: boolean;
  onPlay: (game: Game) => void;
  onOpen: (game: Game) => void;
}

/** 440 px hero: the resume point's screenshot (where Continue picks up), else the
 *  libretro snap, shown sharp (pixelated upscale); without either, the tile colour with
 *  a big monogram. 70 % dark bottom panel with eyebrow, 48 px title, meta,
 *  Continue/Play and View details. */
export function HomeHeroBanner({ game, reason, sessionBusy, onPlay, onOpen }: HomeHeroBannerProps) {
  const { data: systems } = useSystems();
  const { data: resume } = useResumeInfo(game.id);
  const snap =
    (resume?.available ? assetUrl(resume.thumbnailPath, resume.createdAt) : null) ?? assetUrl(game.heroPath);
  const color = tileColor(game.title);
  const played = game.lastPlayedAt !== null;
  return (
    <div className="relative h-[440px] min-w-0 flex-[999_1_560px] overflow-hidden rounded-[18px]" style={{ backgroundColor: color }}>
      {snap ? (
        <img
          src={snap}
          alt=""
          aria-hidden="true"
          className="absolute inset-0 size-full object-cover [image-rendering:pixelated]"
        />
      ) : (
        <>
          <div aria-hidden="true" className="absolute top-10 right-12 size-[360px] rounded-full bg-white/10" />
          <div aria-hidden="true" className="absolute top-[120px] right-[120px] font-display text-[150px] font-bold tracking-[-0.04em] text-white/20">
            {monogram(game.title)}
          </div>
        </>
      )}
      <div className="absolute inset-x-0 bottom-0 flex flex-col gap-3.5 bg-[rgba(10,10,16,0.7)] px-11 py-10">
        <span className="text-xs font-bold tracking-[0.12em] text-link">{heroEyebrow(reason)}</span>
        <h1 className="m-0 line-clamp-2 font-display text-5xl font-bold tracking-[-0.02em]">{game.title}</h1>
        <div className="flex flex-wrap gap-[18px] text-sm text-text-soft">
          <span>{homeMetaLine(systemLabel(systems, game.platformId), game)}</span>
          {played && <span>{formatPlayTime(game.totalPlayS)} played</span>}
          {game.lastPlayedAt !== null && <span>Last played {relativeTimeInSentence(game.lastPlayedAt)}</span>}
        </div>
        <div className="mt-1.5 flex flex-wrap gap-3">
          <button
            type="button"
            onClick={() => onPlay(game)}
            disabled={game.missing || sessionBusy}
            className="inline-flex h-12 cursor-pointer items-center gap-2.5 rounded-control border-0 bg-accent-strong px-6 font-body text-[15px] font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M7 4v16l13-8z" />
            </svg>
            {played ? "Continue" : "Play"}
          </button>
          <button
            type="button"
            onClick={() => onOpen(game)}
            className="inline-flex h-12 cursor-pointer items-center rounded-control border-0 bg-white/12 px-6 font-body text-[15px] font-bold text-white hover:bg-white/20"
          >
            View details
          </button>
        </div>
      </div>
    </div>
  );
}
