import type { GameSort } from "../api/tauri-commands";

const SORTS: { value: GameSort; label: string }[] = [
  { value: "recently-played", label: "Recently played" },
  { value: "title", label: "A–Z" },
  { value: "play-time", label: "Play time" },
  { value: "recently-added", label: "Recently added" },
];

interface LibrarySortSelectProps {
  value: GameSort;
  onChange: (sort: GameSort) => void;
}

/** Sort order, styled like the Genre select (same row as the System and Genre selects). */
export function LibrarySortSelect({ value, onChange }: LibrarySortSelectProps) {
  return (
    <label className="flex h-10 items-center gap-2 rounded-[20px] border border-border-strong px-4 text-sm text-text-soft">
      Sort
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as GameSort)}
        className="cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-text outline-none"
      >
        {SORTS.map((option) => (
          <option key={option.value} value={option.value} className="bg-surface-raised">
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}
