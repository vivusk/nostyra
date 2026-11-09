import { useCloudGameStatus, useCloudStatus, useOpenCloudConflict } from "../../hooks/use-cloud-queries";

const CHIP = "inline-flex h-7 items-center gap-1.5 rounded-full border px-3 font-body text-xs font-bold";

/** Detail → Save states header chip: Synced / Upload pending / Conflict (button: opens
 *  the conflict dialog) / Sync error (reason as tooltip + accessible description).
 *  Hidden while signed out (or the game has nothing to sync). */
export function CloudSyncChip({ gameId }: { gameId: number }) {
  const { data: cloud } = useCloudStatus();
  const signedIn = cloud?.signedIn ?? false;
  const { data: chip } = useCloudGameStatus(gameId, signedIn);
  const openConflict = useOpenCloudConflict();

  if (!signedIn || !chip || chip.state === "off") return null;
  if (chip.state === "conflict") {
    return (
      <button
        type="button"
        onClick={() => void openConflict(gameId)}
        className={`${CHIP} cursor-pointer border-warning/60 bg-transparent text-warning hover:text-white`}
      >
        <Dot className="bg-warning" />
        Cloud conflict — choose
      </button>
    );
  }
  if (chip.state === "error") {
    const reason = chip.error ?? "The last sync failed";
    return (
      <span role="note" tabIndex={0} title={reason} aria-label={`Cloud sync error: ${reason}`} className={`${CHIP} border-warning/60 text-warning`}>
        <Dot className="bg-warning" />
        Sync error
      </span>
    );
  }
  const pending = chip.state === "pending";
  return (
    <span className={`${CHIP} border-border-strong text-text-soft`}>
      <Dot className={pending ? "bg-muted" : "bg-accent"} />
      {pending ? "Upload pending" : "Synced"}
    </span>
  );
}

function Dot({ className }: { className: string }) {
  return <span aria-hidden="true" className={`size-2 rounded-full ${className}`} />;
}
