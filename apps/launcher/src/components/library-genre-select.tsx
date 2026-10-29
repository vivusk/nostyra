import { useEffect } from "react";
import { useGenres } from "../hooks/use-game-detail-queries";

interface LibraryGenreSelectProps {
  /** Empty = all genres. */
  value: string;
  onChange: (genre: string) => void;
}

/** Genre filter from the library's metadata; hidden when no game has a genre. */
export function LibraryGenreSelect({ value, onChange }: LibraryGenreSelectProps) {
  const { data: genres } = useGenres();
  // A genre that no longer exists (metadata edit) must not keep filtering invisibly.
  useEffect(() => {
    if (genres && value !== "" && !genres.includes(value)) onChange("");
  }, [genres, value, onChange]);
  if (!genres || genres.length === 0) return null;
  return (
    <label className="flex h-10 items-center gap-2 rounded-[20px] border border-border-strong px-4 text-sm text-text-soft">
      Genre
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="max-w-48 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-text outline-none"
      >
        <option value="" className="bg-surface-raised">
          All
        </option>
        {genres.map((genre) => (
          <option key={genre} value={genre} className="bg-surface-raised">
            {genre}
          </option>
        ))}
      </select>
    </label>
  );
}
