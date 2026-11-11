import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { GameDetail } from "../../api/tauri-commands-artwork";
import type { NetplayJoinRole } from "../../api/tauri-commands-netplay";
import { capabilities } from "../../backend";
import { playOnlineState } from "../../lib/netplay-rules";
import { NetplayHostDialog } from "./netplay-host-dialog";
import { NetplayJoinDialog } from "./netplay-join-dialog";

type MenuDialog = { kind: "host" } | { kind: "join"; role: NetplayJoinRole } | null;

const ITEM =
  "flex w-full cursor-pointer items-center rounded-control border-0 bg-transparent px-3 py-2.5 text-left font-body text-sm font-semibold text-text-soft hover:bg-nav-hover hover:text-white focus:bg-nav-hover focus:outline-none";

/** Detail header "Play online ▾" (desktop, systems the registry offers): Host game,
 *  Join with code, Watch with code. Disabled with a reason when offline, the ROM is
 *  missing or a game runs; Experimental systems show a badge. */
export function PlayOnlineMenu({ game, sessionRunning }: { game: GameDetail; sessionRunning: boolean }) {
  // Menu anchor in viewport coordinates; null = closed. The menu is portalled to <body>
  // with fixed positioning because the detail hero clips overflowing children.
  const [anchor, setAnchor] = useState<{ top: number; left: number } | null>(null);
  const open = anchor !== null;
  const [dialog, setDialog] = useState<MenuDialog>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const { visible, disabledReason } = playOnlineState(capabilities.netplay, game, sessionRunning);
  const setOpen = (next: boolean) => {
    const rect = next ? rootRef.current?.getBoundingClientRect() : undefined;
    setAnchor(rect ? { top: rect.bottom + 6, left: rect.left } : null);
  };

  // Click outside / Escape / scroll / resize closes the menu (fixed position would drift).
  useEffect(() => {
    if (!open) return;
    const close = (e: Event) => {
      const target = e.target as Node;
      const inside = rootRef.current?.contains(target) || menuRef.current?.contains(target);
      const shouldClose =
        e instanceof KeyboardEvent ? e.key === "Escape" : e.type === "mousedown" ? !inside : true;
      if (shouldClose) setAnchor(null);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", close);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [open]);

  if (!visible) return null;
  const choose = (next: MenuDialog) => () => {
    setOpen(false);
    setDialog(next);
  };

  return (
    <div ref={rootRef} className="relative flex items-center gap-2">
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabledReason !== null}
        title={disabledReason ?? undefined}
        onClick={() => setOpen(!open)}
        className="inline-flex h-14 cursor-pointer items-center gap-2 rounded-card border border-border-strong bg-[rgba(10,10,16,0.6)] px-6 font-body text-[15px] font-bold text-text hover:border-accent disabled:cursor-default disabled:opacity-60"
      >
        Play online
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" aria-hidden="true">
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>
      {game.netplay?.support === "experimental" && (
        <span title="Online play for this system may desync" className="rounded-md bg-warning/15 px-2 py-0.5 text-[11px] font-bold text-warning">
          Experimental
        </span>
      )}
      {anchor && createPortal(
        <div
          ref={menuRef}
          role="menu"
          style={{ top: anchor.top, left: anchor.left }}
          className="fixed z-50 flex w-56 flex-col gap-0.5 rounded-control border border-border-strong bg-surface-raised p-1.5 shadow-xl"
        >
          <button type="button" role="menuitem" autoFocus onClick={choose({ kind: "host" })} className={ITEM}>
            Host game…
          </button>
          <button type="button" role="menuitem" onClick={choose({ kind: "join", role: "player" })} className={ITEM}>
            Join with code…
          </button>
          <button type="button" role="menuitem" onClick={choose({ kind: "join", role: "spectator" })} className={ITEM}>
            Watch with code…
          </button>
        </div>,
        document.body,
      )}
      {dialog?.kind === "host" && <NetplayHostDialog game={game} onClose={() => setDialog(null)} />}
      {dialog?.kind === "join" && <NetplayJoinDialog initialRole={dialog.role} onClose={() => setDialog(null)} />}
    </div>
  );
}
