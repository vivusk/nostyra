import { useState, type ReactNode } from "react";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import { useArtworkProgress, useStartArtworkFetch } from "../../hooks/use-artwork-queries";
import { useRevealRomInFolder } from "../../hooks/use-game-detail-queries";
import { useRemoveMissingGames } from "../../hooks/use-library-queries";
import { formatDateTime, formatPlayTime } from "../../lib/game-display";
import { MISSING_REMOVAL_WARNING } from "../../lib/missing-game-removal-text";
import { ConfirmDialog } from "../confirm-dialog";

const BUTTON =
  "h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";
/** Same shape, red: removes data for good. */
const DANGER_BUTTON =
  "h-11 cursor-pointer rounded-control border border-danger/50 bg-transparent px-4 font-body font-semibold text-danger hover:border-danger hover:bg-danger/10 disabled:cursor-default disabled:opacity-50";

function Row({ label, children, title }: { label: string; children: ReactNode; title?: string }) {
  return (
    <div className="flex justify-between gap-4 border-b border-[#22222A] py-3 text-sm">
      <span className="text-muted">{label}</span>
      <span className="text-right break-all" title={title}>
        {children}
      </span>
    </div>
  );
}

interface DetailInfoPanelProps {
  game: GameDetail;
  onEdit: () => void;
  /** After "Remove from library" succeeded (the page leaves Detail). */
  onRemoved: () => void;
}

/** Info panel: play stats, core, metadata, file, CRC32, and the game actions
 *  (Edit metadata, Open folder, Fetch artwork — the latter only for No-Intro-matched
 *  games, which have a name to look up — and Remove from library for missing ROMs). */
export function DetailInfoPanel({ game, onEdit, onRemoved }: DetailInfoPanelProps) {
  const meta = game.metadata;
  const reveal = useRevealRomInFolder();
  const remove = useRemoveMissingGames();
  const [confirmRemove, setConfirmRemove] = useState(false);
  const startFetch = useStartArtworkFetch();
  const { data: progress } = useArtworkProgress();
  const fetching = startFetch.isPending || Boolean(progress);
  const kinds = game.artwork.filter((a) => a.kind === "cover" || a.kind === "hero");
  const nothingFound = kinds.length === 2 && kinds.every((a) => a.status === "not-found");

  return (
    <aside aria-label="Game info" className="flex max-w-[380px] flex-[1_1_300px] flex-col">
      <Row label="Play time">{game.stats.totalPlayS > 0 ? formatPlayTime(game.stats.totalPlayS) : "Not played"}</Row>
      <Row label="Last played">{game.stats.lastPlayedAt !== null ? formatDateTime(game.stats.lastPlayedAt) : "Never"}</Row>
      <Row label="Core">{[game.coreName, game.coreVersion].filter(Boolean).join(" ")}</Row>
      {meta?.developer && <Row label="Developer">{meta.developer}</Row>}
      {meta?.publisher && <Row label="Publisher">{meta.publisher}</Row>}
      {game.region && <Row label="Region">{game.region}</Row>}
      {meta?.players != null && <Row label="Players">{meta.players}</Row>}
      <Row label="File" title={game.romPath}>
        {game.romFileName}
      </Row>
      <Row label="CRC32">
        <span className="font-mono">{game.crc32}</span>
      </Row>
      <div className="flex flex-wrap gap-2.5 pt-[18px]">
        <button type="button" onClick={onEdit} className={BUTTON}>
          Edit metadata
        </button>
        <button type="button" onClick={() => reveal.mutate(game.id)} disabled={game.missing} className={BUTTON}>
          Open folder
        </button>
        {game.datName !== null && (
          <button type="button" onClick={() => startFetch.mutate(game.id)} disabled={fetching} className={BUTTON}>
            {fetching ? "Fetching artwork…" : "Fetch artwork"}
          </button>
        )}
        {game.missing && (
          <button
            type="button"
            onClick={() => setConfirmRemove(true)}
            disabled={remove.isPending}
            className={DANGER_BUTTON}
          >
            {remove.isPending ? "Removing…" : "Remove from library"}
          </button>
        )}
      </div>
      {game.datName !== null && nothingFound && !fetching && (
        <p className="m-0 pt-2 text-xs text-muted">No artwork found for this game.</p>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title="Remove from library?"
          confirmLabel="Remove"
          danger
          onConfirm={() => remove.mutate(game.id, { onSuccess: onRemoved })}
          onClose={() => setConfirmRemove(false)}
        >
          {MISSING_REMOVAL_WARNING}
        </ConfirmDialog>
      )}
    </aside>
  );
}
