// Hero selector entries for Home (Epic-style, max 4, no duplicates): last played,
// second last played, newest added, a favorite; gaps filled by more played games,
// then more added games. The first entry is the default hero.
import type { Game } from "../api/tauri-commands";
import type { HomeData } from "../api/tauri-commands-artwork";
import { formatPlayTime } from "./game-display";

export type HeroReason = "continue" | "added" | "favorite";

export interface HeroEntry {
  game: Game;
  reason: HeroReason;
}

export const MAX_HERO_ENTRIES = 4;

export function buildHeroEntries(data: HomeData): HeroEntry[] {
  const entries: HeroEntry[] = [];
  const seen = new Set<number>();
  const add = (game: Game | undefined, reason: HeroReason) => {
    if (!game || seen.has(game.id) || entries.length >= MAX_HERO_ENTRIES) return;
    seen.add(game.id);
    entries.push({ game, reason });
  };
  const firstUnseen = (games: Game[]) => games.find((game) => !seen.has(game.id));

  add(data.continuePlaying[0], "continue");
  add(data.continuePlaying[1], "continue");
  add(firstUnseen(data.recentlyAdded), "added");
  add(firstUnseen(data.favorites), "favorite");
  for (const game of data.continuePlaying) add(game, "continue");
  for (const game of data.recentlyAdded) add(game, "added");
  return entries;
}

/** Eyebrow above the hero title. */
export function heroEyebrow(reason: HeroReason): string {
  switch (reason) {
    case "continue":
      return "CONTINUE WHERE YOU LEFT OFF";
    case "added":
      return "RECENTLY ADDED";
    case "favorite":
      return "FAVORITE";
  }
}

/** Selector row subtitle: "Continue · 41h" / "Recently added" / "Favorite". */
export function heroSubtitle(entry: HeroEntry): string {
  switch (entry.reason) {
    case "continue":
      return `Continue · ${formatPlayTime(entry.game.totalPlayS)}`;
    case "added":
      return "Recently added";
    case "favorite":
      return "Favorite";
  }
}
