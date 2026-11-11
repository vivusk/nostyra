import type { NetplayRosterEntry } from "../../api/tauri-events";
import { sortRoster } from "../../lib/netplay-rules";

const ROLE_LABEL: Record<NetplayRosterEntry["role"], string> = {
  host: "Host",
  player: "Player",
  spectator: "Spectator",
};

/** Waiting-room roster: nickname · role · ping · RELAY badge (host first). */
export function NetplayLobbyRoster({ roster }: { roster: NetplayRosterEntry[] }) {
  if (roster.length === 0) return <p className="m-0 text-sm text-muted">Nobody in the room yet.</p>;
  return (
    <ul aria-label="People in the room" className="m-0 flex list-none flex-col gap-1.5 p-0">
      {sortRoster(roster).map((entry, index) => (
        <li key={`${entry.role}-${entry.nick}-${index}`} className="flex items-center gap-3 rounded-control bg-surface-raised px-3 py-2 text-sm">
          <span className="min-w-0 flex-1 truncate font-semibold text-text">{entry.nick}</span>
          <span className="text-xs text-muted">{ROLE_LABEL[entry.role]}</span>
          <span className="w-16 text-right text-xs text-text-soft tabular-nums">{entry.ping_ms !== null ? `${entry.ping_ms} ms` : "—"}</span>
          {entry.relayed && (
            <span title="Connected through the relay server" className="rounded-md bg-[#2A2A35] px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-[#DAD5FF]">
              RELAY
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}
