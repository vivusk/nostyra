import { useEffect, useRef, useState, type FormEvent } from "react";
import type { NetplayJoinRole } from "../../api/tauri-commands-netplay";
import { useNetplayJoin } from "../../hooks/use-netplay-play";
import { normaliseRoomCode, roomErrorText } from "../../lib/netplay-rules";
import { SegmentedControl } from "../gameplay/gameplay-setting-controls";

const BUTTON = "h-11 cursor-pointer rounded-control px-5 font-body font-bold";
const ROLES = [
  { value: "player" as const, label: "Play" },
  { value: "spectator" as const, label: "Watch" },
];

/** Join / Watch with a room code: the host's game must be in this library (found by
 *  its content hash). Refusals stay in the dialog ("Game already started", "Room is
 *  full", "You need … in your library"). */
export function NetplayJoinDialog({ initialRole, onClose }: { initialRole: NetplayJoinRole; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [code, setCode] = useState("");
  const [role, setRole] = useState<NetplayJoinRole>(initialRole);
  const [error, setError] = useState<string | null>(null);
  const join = useNetplayJoin();

  useEffect(() => dialogRef.current?.showModal(), []);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const clean = normaliseRoomCode(code);
    if (!clean) {
      setError("Room codes have 6 letters or digits.");
      return;
    }
    setError(null);
    join.mutate({ code: clean, role }, { onSuccess: onClose, onError: (err) => setError(roomErrorText(err)) });
  };

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="netplay-join-title"
      className="m-auto w-[min(440px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <form onSubmit={submit} className="flex flex-col gap-4 p-7">
        <h2 id="netplay-join-title" className="m-0 font-display text-[22px] font-semibold">
          {role === "player" ? "Join with code" : "Watch with code"}
        </h2>
        <label className="flex flex-col gap-2 text-sm font-semibold">
          Room code
          <input
            autoFocus
            value={code}
            onChange={(e) => setCode(e.target.value)}
            maxLength={12}
            spellCheck={false}
            autoComplete="off"
            placeholder="K7Q-2ZP"
            aria-invalid={error !== null}
            className="h-12 rounded-control border border-border-strong bg-surface-raised px-4 font-display text-xl tracking-[0.2em] text-text uppercase outline-none focus:border-accent"
          />
        </label>
        <div className="flex flex-col gap-2">
          <span id="netplay-role-label" className="text-sm font-semibold">
            Join as
          </span>
          <SegmentedControl controlId="netplay-role" choices={ROLES} value={role} onChange={(value) => value && setRole(value)} />
        </div>
        {error && (
          <p role="alert" className="m-0 text-sm text-danger">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-3">
          <button type="button" onClick={onClose} className={`${BUTTON} border-0 bg-transparent text-text-soft hover:text-white`}>
            Cancel
          </button>
          <button
            type="submit"
            disabled={join.isPending}
            className={`${BUTTON} border-0 bg-accent-strong text-white hover:bg-accent disabled:cursor-default disabled:opacity-60`}
          >
            {join.isPending ? "Joining…" : role === "player" ? "Join" : "Watch"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
