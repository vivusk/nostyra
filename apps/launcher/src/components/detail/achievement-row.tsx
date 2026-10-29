import type { AchievementRow as Row } from "../../api/tauri-commands-achievements";
import { assetUrl } from "../../lib/asset-url";

/** One achievement: 64 px badge (cached file; locked = `_lock` file or greyed colour
 *  badge; none yet = trophy tile), title, description, points, unlock date / mode,
 *  measured progress for locked ones. */
export function AchievementRow({ row }: { row: Row }) {
  const badge = assetUrl(row.badgePath);
  const unlockedOn =
    row.unlocked && row.unlockTime > 0
      ? new Date(row.unlockTime * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })
      : null;
  return (
    <li className={`flex items-center gap-4 rounded-control bg-surface-raised p-3 ${row.unlocked ? "" : "opacity-80"}`}>
      {badge ? (
        <img
          src={badge}
          alt=""
          aria-hidden="true"
          className={`size-16 shrink-0 rounded-md ${row.badgeGrayscale ? "[filter:grayscale(1)_brightness(.7)]" : ""}`}
        />
      ) : (
        <div aria-hidden="true" className="flex size-16 shrink-0 items-center justify-center rounded-md bg-[#2A2A35] text-muted">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="M8 4h8v5a4 4 0 0 1-8 0V4zM8 6H5a3 3 0 0 0 3 4M16 6h3a3 3 0 0 1-3 4M12 13v4M9 20h6" />
          </svg>
        </div>
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-semibold text-text">{row.title}</span>
        <span className="text-sm text-text-soft">{row.description}</span>
        <span className="text-xs text-muted">
          {unlockedOn
            ? `Unlocked ${unlockedOn}${row.unlockedHardcore ? " · Hardcore" : ""}`
            : row.measuredProgress
              ? `Locked · ${row.measuredProgress}`
              : "Locked"}
        </span>
      </div>
      <span className="shrink-0 rounded-md bg-[#2A2A35] px-2 py-1 text-xs font-bold text-[#DAD5FF]">
        {row.points} pts
      </span>
    </li>
  );
}
