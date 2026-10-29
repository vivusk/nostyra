import type { GameDetail } from "../../api/tauri-commands-artwork";
import { useSystems } from "../../hooks/use-system-queries";

const BADGE = "rounded-md bg-[#2A2A35] px-2.5 py-1 text-xs font-bold";

/** System name + identification state (NO-INTRO VERIFIED / NEEDS REVIEW) + MISSING. */
export function DetailBadges({ game }: { game: GameDetail }) {
  const { data: systems } = useSystems();
  const system = systems?.find((s) => s.id === game.platformId)?.displayName ?? game.platformId;
  return (
    <div className="flex flex-wrap gap-2">
      <span className={`${BADGE} text-[#DAD5FF]`}>{system.toUpperCase()}</span>
      {game.datName !== null ? (
        <span className={`${BADGE} text-text-soft`}>NO-INTRO VERIFIED</span>
      ) : (
        <span className="rounded-md bg-warning px-2.5 py-1 text-xs font-bold text-[#1A1406]">NEEDS REVIEW</span>
      )}
      {game.missing && <span className={`${BADGE} text-white`}>MISSING</span>}
    </div>
  );
}
