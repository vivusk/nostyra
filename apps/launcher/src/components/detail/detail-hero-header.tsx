import type { GameDetail } from "../../api/tauri-commands-artwork";
import { useResumeInfo } from "../../hooks/use-resume-info-query";
import { assetUrl } from "../../lib/asset-url";
import { detailMetaLine, monogram, tileColor } from "../../lib/game-display";
import { CoverImage } from "../cover-image";
import { DetailBadges } from "./detail-badges";
import { FavoriteToggleButton } from "./favorite-toggle-button";

interface DetailHeroHeaderProps {
  game: GameDetail;
  /** Label of the back pill ("Home" / "Library"). */
  backLabel: string;
  onBack: () => void;
  /** The running/launching game, if any. */
  sessionGameId: number | null;
  onPlay: () => void;
}

/** 460 px header per mockup: blurred snap (else tile colour + big monogram), back pill,
 *  overlapping 150 px cover (letterboxed), badges, 52 px title, meta, Play + heart. */
export function DetailHeroHeader({ game, backLabel, onBack, sessionGameId, onPlay }: DetailHeroHeaderProps) {
  const snap = assetUrl(game.heroPath);
  const meta = game.metadata;
  const metaLine = detailMetaLine({
    developer: meta?.developer ?? null,
    releaseYear: meta?.releaseYear ?? null,
    genre: meta?.genre ?? null,
    players: meta?.players ?? null,
  });
  const running = sessionGameId === game.id;
  const disabledReason = game.missing ? "The ROM file is missing" : sessionGameId !== null && !running ? "Another game is running" : null;
  const playLabel = running ? "Running…" : game.lastPlayedAt !== null ? "Continue" : "Play";
  const { data: resume } = useResumeInfo(game.id);

  return (
    <header className="relative h-[460px] overflow-hidden" style={{ backgroundColor: tileColor(game.title) }}>
      {snap ? (
        <img src={snap} alt="" aria-hidden="true" className="absolute inset-0 size-full scale-110 object-cover [filter:blur(18px)_brightness(.55)]" />
      ) : (
        <>
          <div aria-hidden="true" className="absolute top-10 right-[8%] size-[420px] rounded-full bg-white/10" />
          <div aria-hidden="true" className="absolute top-[140px] right-[12%] font-display text-[180px] font-bold tracking-[-0.04em] text-white/20">
            {monogram(game.title)}
          </div>
        </>
      )}
      <button
        type="button"
        onClick={onBack}
        className="absolute top-6 left-10 inline-flex h-11 cursor-pointer items-center gap-2 rounded-[22px] border-0 bg-[rgba(10,10,16,0.6)] px-4 font-body font-semibold text-text"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" aria-hidden="true">
          <path d="M15 18l-6-6 6-6" />
        </svg>
        {backLabel}
      </button>
      <div className="absolute inset-x-0 bottom-0 flex flex-wrap items-end gap-7 bg-[rgba(10,10,16,0.78)] px-10 py-9">
        <CoverImage
          src={assetUrl(game.coverPath, game.coverCheckedAt)}
          title={game.title}
          className="-mt-[90px] aspect-[3/4] w-[150px] rounded-card border border-white/10"
        />
        <div className="flex min-w-0 flex-[1_1_360px] flex-col gap-3">
          <DetailBadges game={game} />
          <h1 className="m-0 line-clamp-2 font-display text-[52px] leading-tight font-bold tracking-[-0.02em]">{game.title}</h1>
          {metaLine && <span className="text-text-soft">{metaLine}</span>}
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={onPlay}
            disabled={running || disabledReason !== null}
            title={disabledReason ?? undefined}
            className="inline-flex h-14 cursor-pointer items-center gap-3 rounded-card border-0 bg-accent-strong px-9 font-body text-[17px] font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-60"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M7 4v16l13-8z" />
            </svg>
            {playLabel}
          </button>
          <FavoriteToggleButton gameId={game.id} favorite={game.favorite} />
          {resume?.hardcore && <span className="basis-full text-xs text-muted">Hardcore: resume disabled</span>}
        </div>
      </div>
    </header>
  );
}
