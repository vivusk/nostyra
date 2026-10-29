import { useState } from "react";
import { AchievementsTab } from "../components/detail/achievements-tab";
import { DetailAboutSection } from "../components/detail/detail-about-section";
import { DetailHeroHeader } from "../components/detail/detail-hero-header";
import { DetailInfoPanel } from "../components/detail/detail-info-panel";
import { DETAIL_TABS, DetailTabs, detailTabPanelId, type DetailTab } from "../components/detail/detail-tabs";
import { EditMetadataDialog } from "../components/detail/edit-metadata-dialog";
import { GameSettingsTab } from "../components/detail/game-settings-tab";
import { SaveStatesTab } from "../components/detail/save-states-tab";
import { useGameDetail } from "../hooks/use-game-detail-queries";
import { useGameSession } from "../hooks/use-game-session";
import { usePlayGame } from "../hooks/use-play-game";
import { useNavigation } from "../navigation/navigation-context";

/** Game Detail per mockup: hero header, tabs (Overview · Save states · Settings ·
 *  Achievements = RetroAchievements snapshot, M15), About + info panel,
 *  Edit metadata dialog. Full-bleed: cancels the shell's content padding. */
export function GameDetailPage({ gameId }: { gameId: number }) {
  const { back, previous } = useNavigation();
  const { data: game, isPending, isError } = useGameDetail(gameId);
  const { data: session } = useGameSession();
  const play = usePlayGame();
  const [tab, setTab] = useState<DetailTab>("Overview");
  const [editing, setEditing] = useState(false);
  const backLabel = previous?.page === "home" ? "Home" : "Library";

  if (isPending) return null;
  if (!game) {
    return (
      <div className="flex flex-col items-start gap-4">
        <p className="m-0 text-text-soft">
          {isError ? "This game could not be loaded." : "This game is no longer in the library."}
        </p>
        <button type="button" onClick={back} className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white">
          ← {backLabel}
        </button>
      </div>
    );
  }

  return (
    <div className="-mx-10 -mt-8 -mb-14">
      <DetailHeroHeader
        game={game}
        backLabel={backLabel}
        onBack={back}
        sessionGameId={session?.gameId ?? null}
        onPlay={() => play(game.id)}
      />
      <DetailTabs value={tab} onChange={setTab} />
      <div className="flex max-w-[1360px] flex-wrap gap-10 px-10 pt-8 pb-14">
        <div
          id={detailTabPanelId}
          role="tabpanel"
          aria-labelledby={`detail-tab-${DETAIL_TABS.indexOf(tab)}`}
          className="flex min-w-0 flex-[999_1_560px] flex-col gap-8">
          {tab === "Overview" && <DetailAboutSection description={game.metadata?.description ?? null} />}
          {tab === "Save states" && <SaveStatesTab game={game} />}
          {tab === "Settings" && <GameSettingsTab gameId={game.id} platformId={game.platformId} />}
          {tab === "Achievements" && <AchievementsTab gameId={game.id} />}
        </div>
        <DetailInfoPanel game={game} onEdit={() => setEditing(true)} onRemoved={back} />
      </div>
      {editing && <EditMetadataDialog game={game} onClose={() => setEditing(false)} />}
    </div>
  );
}
