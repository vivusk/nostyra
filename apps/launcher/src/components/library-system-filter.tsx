import { useEffect } from "react";
import { useLibrarySystems } from "../hooks/use-system-queries";

interface LibrarySystemFilterProps {
  /** Platform id; empty = all systems. */
  value: string;
  onChange: (system: string) => void;
}

/** Up to this many options ("All systems" included) show as pills; more become a select. */
const MAX_PILL_OPTIONS = 4;

/**
 * System filter: "All · SNES · NES" pills, or a "System" select once the library has
 * games of more systems than fit as pills. Hidden while the library has games of ≤ 1
 * system.
 */
export function LibrarySystemFilter({ value, onChange }: LibrarySystemFilterProps) {
  const { data } = useLibrarySystems();
  // A system whose last game is gone must not keep filtering invisibly.
  useEffect(() => {
    if (data && value !== "" && !data.some((s) => s.id === value)) onChange("");
  }, [data, value, onChange]);
  const systems = data ?? [];
  if (systems.length <= 1) return null;
  const options = [{ id: "", label: "All systems" }, ...systems.map((s) => ({ id: s.id, label: s.shortLabel }))];
  if (options.length > MAX_PILL_OPTIONS) {
    return (
      <label className="flex h-10 items-center gap-2 rounded-[20px] border border-border-strong px-4 text-sm text-text-soft">
        System
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="max-w-48 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-text outline-none"
        >
          {options.map((option) => (
            <option key={option.id || "all"} value={option.id} className="bg-surface-raised">
              {option.id ? option.label : "All"}
            </option>
          ))}
        </select>
      </label>
    );
  }
  return (
    <div role="group" aria-label="Systems" className="flex flex-wrap gap-2.5">
      {options.map((option) => {
        const active = option.id === value;
        return (
          <button
            key={option.id || "all"}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(option.id)}
            className={`h-10 cursor-pointer rounded-[20px] border px-4 font-body text-sm font-bold ${
              active
                ? "border-accent bg-accent text-app-bg"
                : "border-border-strong bg-transparent text-text-soft hover:border-[#4A4A55] hover:text-white"
            }`}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
