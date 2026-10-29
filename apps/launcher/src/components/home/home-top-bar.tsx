import { useState } from "react";
import { useNavigation } from "../../navigation/navigation-context";
import { AddFolderButton } from "../add-folder-button";

/** Home top bar: search hands off to the Library on the first keystroke (text and
 *  focus carried over, so typing never gets yanked mid-word), plus Add games. */
export function HomeTopBar() {
  const { navigate, updateLibrary } = useNavigation();
  const [text, setText] = useState("");

  const openLibrary = (search: string) => {
    // A fresh search over the whole library (no leftover filter or genre).
    updateLibrary({ search, filter: "all", genre: "", scrollOffset: 0 });
    navigate({ page: "library", focusSearch: true });
  };

  return (
    <div className="flex flex-wrap items-center gap-4">
      <label className="flex h-11 max-w-[420px] min-w-[280px] flex-1 items-center gap-2.5 rounded-[22px] bg-surface-raised px-4 text-muted-dim">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-3.5-3.5" />
        </svg>
        <span className="sr-only">Search library</span>
        <input
          type="search"
          value={text}
          placeholder="Search your library"
          onChange={(e) => {
            setText(e.target.value);
            if (e.target.value.trim()) openLibrary(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") openLibrary(text);
          }}
          className="flex-1 border-0 bg-transparent font-body text-[15px] text-text outline-none placeholder:text-muted-dim"
        />
      </label>
      <div className="ml-auto">
        <AddFolderButton label="Add games" />
      </div>
    </div>
  );
}
