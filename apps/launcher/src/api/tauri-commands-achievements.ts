// RetroAchievements commands (src-tauri/src/commands/achievements_{commands,game_view,mode}.rs).
// The password goes to `raLogin` once; only username + token are stored by the backend.
import { invoke } from "@tauri-apps/api/core";

/** The logged-in account; points are known only after a login in this launcher run. */
export interface RaAccount {
  user: string;
  points: number | null;
  softcorePoints: number | null;
}

/** What this build offers (the hardcore toggle stays hidden until RA approves Nostyra). */
export interface AchievementsCapabilities {
  hardcoreUi: boolean;
}

export type AchievementsStatus = "unsupported" | "loggedOut" | "notLoaded" | "ready";

export interface AchievementsHeader {
  title: string;
  /** Cached game icon (absolute path for `assetUrl`). */
  iconPath: string | null;
  /** Mode of the session that wrote the snapshot. */
  hardcore: boolean;
  /** Unix seconds. */
  updatedAt: number;
  unlocked: number;
  total: number;
  pointsUnlocked: number;
  pointsTotal: number;
}

export interface AchievementRow {
  id: number;
  title: string;
  description: string;
  points: number;
  unlocked: boolean;
  unlockedHardcore: boolean;
  /** Unix seconds; 0 while locked. */
  unlockTime: number;
  /** "5/10"; empty when not measured. */
  measuredProgress: string;
  /** Cached badge (absolute path for `assetUrl`); null = not downloaded yet. */
  badgePath: string | null;
  /** Locked row showing the colour badge: grey it in CSS. */
  badgeGrayscale: boolean;
}

/** Game Detail → Achievements tab (runner snapshot; unlocked rows first). */
export interface GameAchievementsView {
  status: AchievementsStatus;
  systemName: string;
  /** Settings → RetroAchievements → Enabled. */
  enabled: boolean;
  game: AchievementsHeader | null;
  achievements: AchievementRow[];
}

export const achievementsCommands = {
  /** Rejects with kind "login" (wrong password, refused), "network" or "invalid". */
  raLogin: (username: string, password: string) => invoke<RaAccount>("ra_login", { username, password }),
  raLogout: () => invoke<void>("ra_logout"),
  raAccount: () => invoke<RaAccount | null>("ra_account"),
  capabilities: () => invoke<AchievementsCapabilities>("achievements_capabilities"),
  gameAchievements: (gameId: number) => invoke<GameAchievementsView>("game_achievements", { gameId }),
};
