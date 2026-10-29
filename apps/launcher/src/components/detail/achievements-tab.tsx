import type { ReactNode } from "react";
import type { GameAchievementsView } from "../../api/tauri-commands-achievements";
import { useGameAchievements } from "../../hooks/use-achievements-queries";
import { assetUrl } from "../../lib/asset-url";
import { relativeTimeInSentence } from "../../lib/relative-time";
import { AchievementRow } from "./achievement-row";

const CARD = "flex max-w-[720px] flex-col gap-4 rounded-[14px] border border-border bg-surface p-7";

/** Detail → Achievements: the runner's last snapshot for this game (header + rows,
 *  unlocked first), or why there is none. Refreshed by `achievements-changed` and at
 *  session end. Nothing is fetched from the web here. */
export function AchievementsTab({ gameId }: { gameId: number }) {
  const { data: view, isError } = useGameAchievements(gameId);
  if (!view) return isError ? <Message>The achievements for this game could not be loaded.</Message> : null;

  switch (view.status) {
    case "unsupported":
      return <Message>RetroAchievements is not available for {view.systemName}.</Message>;
    case "loggedOut":
      return <Message>Log in to RetroAchievements under Settings to track achievements for this game.</Message>;
    case "notLoaded":
      return (
        <Message>
          Play this game once while logged in to load its achievements.
          {!view.enabled && " Achievements are turned off in Settings."}
        </Message>
      );
    case "ready":
      return <AchievementList view={view} />;
  }
}

function AchievementList({ view }: { view: GameAchievementsView }) {
  const game = view.game;
  if (!game) return null;
  const icon = assetUrl(game.iconPath);
  return (
    <section aria-labelledby="achievements-title" className={CARD}>
      <div className="flex flex-wrap items-center gap-4">
        {icon && <img src={icon} alt="" aria-hidden="true" className="size-14 rounded-md" />}
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <h2 id="achievements-title" className="m-0 font-display text-[22px] font-semibold">
            {game.unlocked} of {game.total} unlocked
          </h2>
          <span className="text-sm text-muted">
            {game.pointsUnlocked} / {game.pointsTotal} points · {game.hardcore ? "Hardcore" : "Softcore"} · updated{" "}
            {relativeTimeInSentence(game.updatedAt)}
          </span>
        </div>
      </div>
      {!view.enabled && <p className="m-0 text-sm text-warning">Achievements are turned off in Settings.</p>}
      {view.achievements.length === 0 ? (
        <p className="m-0 text-text-soft">This game has no achievements yet.</p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {view.achievements.map((row) => (
            <AchievementRow key={row.id} row={row} />
          ))}
        </ul>
      )}
    </section>
  );
}

function Message({ children }: { children: ReactNode }) {
  return (
    <section className={CARD}>
      <h2 className="m-0 font-display text-[22px] font-semibold">Achievements</h2>
      <p className="m-0 leading-relaxed text-text-soft">{children}</p>
    </section>
  );
}
