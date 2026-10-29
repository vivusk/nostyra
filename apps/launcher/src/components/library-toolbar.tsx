interface LibraryToolbarProps {
  /** Focused on mount (Home search hands typing over to the Library). */
  autoFocusSearch?: boolean;
  search: string;
  onSearchChange: (search: string) => void;
  gameCount: number;
  /** A scan is running: Rescan stays in place, disabled, so the header does not jump. */
  scanning: boolean;
  onRescan: () => void;
}

/** Search + Rescan (mockup header right side; stays visible on narrow windows). */
export function LibraryToolbar({ autoFocusSearch, search, onSearchChange, gameCount, scanning, onRescan }: LibraryToolbarProps) {
  return (
    // Narrow (stacked under the title): search fills the row. Wide: fixed width, pushed right.
    <div className="flex items-center gap-2.5 @3xl:ml-auto">
      <label className="flex h-11 min-w-0 flex-1 items-center gap-2.5 rounded-[22px] bg-surface-raised px-4 text-muted-dim @3xl:w-[300px] @3xl:flex-none">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-3.5-3.5" />
        </svg>
        <span className="sr-only">Search library</span>
        <input
          type="search"
          autoFocus={autoFocusSearch}
          // Typing continues where Home left off: caret after the carried-over text.
          onFocus={
            autoFocusSearch
              ? (e) => e.currentTarget.setSelectionRange(e.currentTarget.value.length, e.currentTarget.value.length)
              : undefined
          }
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder={`Search ${gameCount} games`}
          className="flex-1 border-0 bg-transparent font-body text-[15px] text-text outline-none placeholder:text-muted-dim"
        />
      </label>
      <button
        type="button"
        onClick={onRescan}
        disabled={scanning}
        className="inline-flex h-11 shrink-0 cursor-pointer items-center gap-2 rounded-control border-0 bg-accent-strong px-4 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-60"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M20 11a8 8 0 1 0-2.3 5.7" />
          <path d="M20 4v7h-7" />
        </svg>
        {scanning ? "Scanning…" : "Rescan"}
      </button>
    </div>
  );
}
