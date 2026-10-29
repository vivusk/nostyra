import type { ReactNode } from "react";
import type { LibrarySystem } from "../api/tauri-commands-systems";
import type { RootPage } from "../navigation/route-types";
import { HomeIcon, LibraryIcon, NostyraLogoMark, SettingsIcon } from "./sidebar-icons";

interface SidebarNavigationProps {
  /** Highlighted root page; `null` on Detail. */
  currentPage: RootPage | null;
  onNavigate: (page: RootPage) => void;
  /** Games in the library (all systems). */
  gameCount?: number;
  /** Systems with games; each opens the Library filtered to it. */
  systems: LibrarySystem[];
  /** Library system filter (platform id, empty = all). */
  activeSystem: string;
  onOpenSystem: (systemId: string) => void;
}

/** Left sidebar per mockup: brand, main pages, systems list, settings pinned to the bottom. */
export function SidebarNavigation({
  currentPage,
  onNavigate,
  gameCount,
  systems,
  activeSystem,
  onOpenSystem,
}: SidebarNavigationProps) {
  return (
    <nav
      aria-label="Main"
      className="flex w-[248px] shrink-0 flex-col gap-1.5 overflow-y-auto border-r border-sidebar-border bg-sidebar px-[18px] py-7"
    >
      <div className="flex flex-col gap-1 px-3.5 pb-7">
        <div className="flex items-center gap-2.5">
          <NostyraLogoMark />
          <span className="font-display text-xl font-bold tracking-[0.14em]">NOSTYRA</span>
        </div>
        <span className="pl-[38px] text-xs text-muted-dim">Old Worlds. New Ways.</span>
      </div>

      <NavButton active={currentPage === "home"} onClick={() => onNavigate("home")}>
        <HomeIcon />
        Home
      </NavButton>
      <NavButton active={currentPage === "library"} onClick={() => onNavigate("library")}>
        <LibraryIcon />
        Library
        {gameCount !== undefined && <span className="ml-auto text-[13px] text-muted-dim">{gameCount}</span>}
      </NavButton>

      {systems.length > 0 && (
        <h2 className="mx-3.5 mt-[22px] mb-2 text-xs font-bold tracking-[0.1em] text-muted-dim">SYSTEMS</h2>
      )}
      {systems.map((system) => (
        <NavButton
          key={system.id}
          active={currentPage === "library" && activeSystem === system.id}
          isCurrentPage={false}
          onClick={() => onOpenSystem(system.id)}
        >
          <SystemBadge className="text-accent">{system.shortLabel}</SystemBadge>
          {system.displayName}
          <span className="ml-auto text-[13px] text-muted-dim">{system.count}</span>
        </NavButton>
      ))}

      <div className="mt-auto flex flex-col gap-1.5 pt-6">
        <NavButton active={currentPage === "settings"} onClick={() => onNavigate("settings")}>
          <SettingsIcon />
          Settings
        </NavButton>
      </div>
    </nav>
  );
}

interface NavButtonProps {
  active: boolean;
  /** Set false for secondary entries that only mirror the current page, so aria-current stays unique. */
  isCurrentPage?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
}

/** Real <button>s keep every entry focusable for keyboard and future gamepad navigation. */
function NavButton({ active, isCurrentPage = active, disabled = false, onClick, children }: NavButtonProps) {
  const stateClasses = disabled
    ? "cursor-default text-disabled"
    : active
      ? "bg-nav-active text-white"
      : "cursor-pointer text-text-nav hover:bg-nav-hover hover:text-white";
  return (
    <button
      type="button"
      disabled={disabled}
      aria-current={isCurrentPage ? "page" : undefined}
      onClick={onClick}
      className={`flex min-h-11 w-full items-center gap-3.5 rounded-control border-0 bg-transparent px-3.5 py-3 text-left font-body text-[15px] font-medium ${stateClasses}`}
    >
      {children}
    </button>
  );
}

function SystemBadge({ className = "", children }: { className?: string; children: ReactNode }) {
  return <span className={`w-9 shrink-0 text-center text-[11px] font-bold ${className}`}>{children}</span>;
}
