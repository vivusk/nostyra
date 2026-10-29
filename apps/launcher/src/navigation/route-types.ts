// App routes: state-based (no URL bar, four pages), with a back stack for Detail.
import type { GameFilter, GameSort } from "../api/tauri-commands";

export type Route =
  | { page: "home" }
  /** `focusSearch`: arrived by typing in the Home search box (focus the Library search). */
  | { page: "library"; focusSearch?: boolean }
  | { page: "detail"; gameId: number }
  | { page: "settings" };

export type RootPage = Exclude<Route["page"], "detail">;

/** Library controls + scroll offset, kept across navigation so Back restores them. */
export interface LibraryViewState {
  search: string;
  sort: GameSort;
  filter: GameFilter;
  /** Empty = all genres. */
  genre: string;
  /** Platform id; empty = all systems. */
  system: string;
  scrollOffset: number;
}

export const DEFAULT_LIBRARY_VIEW: LibraryViewState = {
  search: "",
  sort: "recently-played",
  filter: "all",
  genre: "",
  system: "",
  scrollOffset: 0,
};
