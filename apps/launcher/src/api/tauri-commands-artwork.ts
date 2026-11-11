// Typed wrappers for the M2 commands: game detail, favorites, metadata edits, home
// data and the artwork job (src-tauri/src/commands/{game_detail,home,artwork}_commands.rs).
import { invoke } from "../backend";
import type { ArtworkProgressEvent } from "./tauri-events";
import type { Game } from "./tauri-commands";

export interface GameMetadata {
  titleOverride: string | null;
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  releaseYear: number | null;
  players: number | null;
  description: string | null;
  /** Saved by the user: kept until "Reset to database values". */
  userEdited: boolean;
  updatedAt: number;
}

export interface PlayStats {
  totalPlayS: number;
  lastPlayedAt: number | null;
  sessionCount: number;
}

export type ArtworkStatus = "cached" | "not-found" | "failed";

export interface ArtworkRecord {
  gameId: number;
  kind: "cover" | "hero";
  status: ArtworkStatus;
  checkedAt: number;
}

/** `Game` fields plus metadata, stats and display extras. */
export interface GameDetail extends Game {
  metadata: GameMetadata | null;
  stats: PlayStats;
  romFileName: string;
  coreName: string | null;
  coreVersion: string | null;
  artwork: ArtworkRecord[];
  /** Play online facts. On the web these are the desktop app's (the browser cannot
   *  play online: Play online opens a "desktop app only" notice). */
  netplay?: GameNetplayInfo;
}

/** Registry-only Play online support of a game's system. */
export interface GameNetplayInfo {
  /** `experimental`: badge + warning (e.g. PlayStation). */
  support: "yes" | "experimental" | "no";
  /** Largest room the host may open (2–4). */
  maxPlayers: number;
  /** Offline mode (`NOSTYRA_OFFLINE=1`): the menu shows disabled. */
  offline: boolean;
}

/** Edit metadata dialog values; blank strings are saved as "no value". */
export interface UserMetadataEdit {
  title?: string;
  developer?: string;
  publisher?: string;
  genre?: string;
  releaseYear?: number;
  players?: number;
  description?: string;
}

export interface HomeData {
  continuePlaying: Game[];
  recentlyAdded: Game[];
  favorites: Game[];
}

export interface ArtworkCacheStats {
  files: number;
  bytes: number;
}

export const artworkCommands = {
  getGameDetail: (gameId: number) => invoke<GameDetail | null>("get_game_detail", { gameId }),
  setFavorite: (gameId: number, favorite: boolean) => invoke<void>("set_favorite", { gameId, favorite }),
  updateGameMetadata: (gameId: number, edit: UserMetadataEdit) =>
    invoke<GameDetail>("update_game_metadata", { gameId, edit }),
  resetGameMetadata: (gameId: number) => invoke<GameDetail>("reset_game_metadata", { gameId }),
  listGenres: () => invoke<string[]>("list_genres"),
  revealRomInFolder: (gameId: number) => invoke<void>("reveal_rom_in_folder", { gameId }),
  getHomeData: () => invoke<HomeData>("get_home_data"),
  /** All missing art (no id) or one game's (retry window ignored). */
  startArtworkFetch: (gameId?: number) => invoke<void>("start_artwork_fetch", { gameId: gameId ?? null }),
  cancelArtworkFetch: () => invoke<void>("cancel_artwork_fetch"),
  getArtworkProgress: () => invoke<ArtworkProgressEvent | null>("get_artwork_progress"),
  getArtworkCacheStats: () => invoke<ArtworkCacheStats>("get_artwork_cache_stats"),
  clearArtworkCache: () => invoke<void>("clear_artwork_cache"),
  getArtworkAutoFetch: () => invoke<boolean>("get_artwork_auto_fetch"),
  setArtworkAutoFetch: (enabled: boolean) => invoke<void>("set_artwork_auto_fetch", { enabled }),
};
