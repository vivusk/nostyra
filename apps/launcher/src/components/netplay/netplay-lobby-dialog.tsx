import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "../../api/query-keys";
import { netplayCommands } from "../../api/tauri-commands-netplay";
import { useQuitSession } from "../../hooks/use-netplay-play";
import { useNetplayLobby, useSetNetplayLobby, type NetplayLobby } from "../../hooks/use-netplay-session";
import { formatRoomCode, lobbyStatusText } from "../../lib/netplay-rules";
import { NetplayLobbyRoster } from "./netplay-lobby-roster";

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold";

/** The online room once the runner is starting (rendered by the shell, desktop only). */
export function NetplayLobbyHost() {
  const { data: lobby } = useNetplayLobby();
  if (!lobby || lobby.opening) return null;
  return <NetplayLobbyDialog lobby={lobby} />;
}

/** Room code + Copy, roster (nick · role · ping · RELAY), status line (Waiting for
 *  host / Syncing n % / Started / why it ended). Cancel ends the game before the
 *  session started; afterwards Close only hides the dialog. Start is pressed in the
 *  game window (the launcher has no channel to the runner). */
function NetplayLobbyDialog({ lobby }: { lobby: NetplayLobby }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const setLobby = useSetNetplayLobby();
  const quit = useQuitSession();
  const [copied, setCopied] = useState(false);
  const host = lobby.role === "host";
  const pending = lobby.phase.kind === "connecting" || lobby.phase.kind === "waiting" || lobby.phase.kind === "syncing";
  // Until the runner reports the roster, the server's counts show the room is alive.
  const { data: counts } = useQuery({
    queryKey: queryKeys.netplayRoomStatus(lobby.code ?? ""),
    queryFn: () => netplayCommands.roomStatus(lobby.code ?? ""),
    enabled: lobby.code !== null && lobby.phase.kind === "connecting",
    refetchInterval: 5000,
    retry: false,
  });

  useEffect(() => {
    if (!dialogRef.current?.open) dialogRef.current?.showModal();
  }, []);

  const close = () => setLobby(() => null);
  const copy = () => {
    if (!lobby.code) return;
    void navigator.clipboard.writeText(lobby.code).then(() => setCopied(true), () => setCopied(false));
  };

  return (
    <dialog
      ref={dialogRef}
      onCancel={(e) => pending && e.preventDefault()}
      onClose={close}
      aria-labelledby="netplay-lobby-title"
      className="m-auto w-[min(500px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="netplay-lobby-title" className="m-0 font-display text-[22px] font-semibold">
          {host ? "Your online room" : lobby.role === "spectator" ? "Watching online" : "Online room"}
        </h2>
        {lobby.code && (
          <div className="flex items-center gap-3 rounded-control bg-surface-raised px-4 py-3">
            <span className="flex-1">
              <span className="block text-xs text-muted">Room code</span>
              <span className="font-display text-2xl font-semibold tracking-[0.15em]">{formatRoomCode(lobby.code)}</span>
            </span>
            <button type="button" onClick={copy} className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white">
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
        )}
        <p role="status" className="m-0 text-sm font-semibold text-text">
          {lobbyStatusText(lobby.phase, host)}
        </p>
        {counts && lobby.phase.kind === "connecting" && (
          <p className="m-0 text-xs text-muted">
            In the room: {counts.players} of {counts.maxPlayers} players · {counts.spectators} watching
          </p>
        )}
        <NetplayLobbyRoster roster={lobby.roster} />
        {lobby.stats && (
          <p className="m-0 text-xs text-text-soft">
            Ping {lobby.stats.pingMs ?? "—"} ms{lobby.stats.relayed ? " · through the relay" : ""}
          </p>
        )}
        {lobby.experimental && <p className="m-0 text-xs text-warning">Experimental system: the game may desync.</p>}
        {lobby.notice && <p className="m-0 text-xs text-warning">{lobby.notice}</p>}
        <div className="flex justify-end gap-3">
          {pending ? (
            <button
              type="button"
              disabled={quit.isPending}
              onClick={() => quit.mutate()}
              className={`${BUTTON} border border-border-strong bg-transparent text-text-soft hover:text-white disabled:opacity-60`}
            >
              {host ? "Close room" : "Leave"}
            </button>
          ) : (
            <button type="button" autoFocus onClick={close} className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent`}>
              Close
            </button>
          )}
        </div>
      </div>
    </dialog>
  );
}
