// TanStack Query keys. Backend events write into / invalidate these
// (see hooks/use-backend-event-sync.ts).
import type { GameListQuery } from "./tauri-commands";

export const queryKeys = {
  setup: ["setup"] as const,
  folders: ["folders"] as const,
  summary: ["summary"] as const,
  /** Under the summary prefix: refreshed whenever the summary is. */
  librarySystems: ["summary", "systems"] as const,
  systems: ["systems"] as const,
  /** Play refused for a missing core: the install prompt's state (null = closed). */
  coreInstallPrompt: ["core-install-prompt"] as const,
  gamesRoot: ["games"] as const,
  games: (query: GameListQuery) => ["games", query] as const,
  coreStatus: ["core-status"] as const,
  /** Live scan progress (null = idle), seeded by get_scan_progress, updated by events. */
  scanProgress: ["scan-progress"] as const,
  /** Live core download progress (null = idle), event-driven only. */
  coreDownload: ["core-download"] as const,
  /** Current game session (null = none). */
  session: ["session"] as const,
  gameRoot: ["game"] as const,
  game: (gameId: number) => ["game", gameId] as const,
  genres: ["genres"] as const,
  home: ["home"] as const,
  /** Live artwork job progress (null = idle), seeded by get_artwork_progress. */
  artworkProgress: ["artwork-progress"] as const,
  artworkCacheStats: ["artwork-cache-stats"] as const,
  artworkAutoFetch: ["artwork-auto-fetch"] as const,
  gameplayConfigRoot: ["gameplay-config"] as const,
  /** Effective + layer values for one game, or the global defaults (`null`). */
  gameplayConfig: (gameId: number | null) => ["gameplay-config", gameId] as const,
  saveStatesRoot: ["save-states"] as const,
  saveStates: (gameId: number) => ["save-states", gameId] as const,
  /** Binding names, labels and defaults (static for the app's lifetime). */
  inputCatalog: ["input-catalog"] as const,
  /** Label style of the last captured pad. */
  gamepadLabelStyle: ["gamepad-label-style"] as const,
  /** BIOS files and what each region boots, per core with BIOS files. */
  bios: ["bios"] as const,
  memoryCards: (gameId: number) => ["memory-cards", gameId] as const,
  /** Files the last scan could not add (reasons), shown by the scan problems dialog. */
  scanProblems: ["scan-problems"] as const,
  /** Scan problems dialog open (true) or closed. */
  scanProblemsOpen: ["scan-problems-open"] as const,
  /** RetroAchievements login (null = logged out). */
  raAccount: ["ra-account"] as const,
  achievementsCapabilities: ["achievements-capabilities"] as const,
  gameAchievementsRoot: ["game-achievements"] as const,
  /** One game's achievement snapshot view (Detail tab). */
  gameAchievements: (gameId: number) => ["game-achievements", gameId] as const,
  resumeInfoRoot: ["resume-info"] as const,
  resumeInfo: (gameId: number) => ["resume-info", gameId] as const,
};

/** What the UI knows about the running game. */
export interface GameSessionState {
  gameId: number;
  /** "launching" until the runner reports Started (or the launch timeout passes). */
  phase: "launching" | "playing";
}
