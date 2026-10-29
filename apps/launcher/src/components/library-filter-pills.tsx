import type { GameFilter } from "../api/tauri-commands";

const FILTERS: { value: GameFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "recently-played", label: "Recently played" },
  { value: "unplayed", label: "Unplayed" },
  { value: "favorites", label: "Favorites" },
  { value: "missing", label: "Missing" },
];

interface LibraryFilterPillsProps {
  value: GameFilter;
  onChange: (filter: GameFilter) => void;
}

/** Library filter pills (genre is a separate select). */
export function LibraryFilterPills({ value, onChange }: LibraryFilterPillsProps) {
  return (
    <div role="group" aria-label="Filters" className="flex flex-wrap gap-2.5">
      {FILTERS.map((filter) => {
        const active = filter.value === value;
        return (
          <button
            key={filter.value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(filter.value)}
            className={`h-10 cursor-pointer rounded-[20px] border px-4 font-body text-sm font-medium ${
              active
                ? "border-text bg-text text-app-bg"
                : "border-border-strong bg-transparent text-text-soft hover:border-[#4A4A55] hover:text-white"
            }`}
          >
            {filter.label}
          </button>
        );
      })}
    </div>
  );
}
