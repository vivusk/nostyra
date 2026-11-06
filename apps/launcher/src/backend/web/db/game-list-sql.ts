// SQL of the library list, mirroring crates/storage-db/src/game_list_sql.rs (same
// columns, filters, sort keys). Filter/sort fragments come from fixed tables, never
// from user input; search, genre, platform, limit and window are bound as ?1..?5.
import type { GameFilter, GameSort } from "../../../api/tauri-commands";

/** `GAME_SELECT`: game columns + play stats + metadata summary + cached art. */
export const GAME_SELECT = `SELECT g.id, g.platform_id, g.folder_id, g.rom_path, g.file_size, g.crc32, g.sha1,
    COALESCE(m.title_override, g.title) AS title, g.region, g.dat_name, g.added_at, g.missing,
    COALESCE(s.total_play_s, 0) AS total_play_s, s.last_played_at, m.genre, m.release_year, m.developer,
    g.favorite, ac.local_path AS cover_path, ah.local_path AS hero_path, ac.checked_at AS cover_checked_at, g.serial
  FROM games g
  LEFT JOIN game_stats s ON s.game_id = g.id
  LEFT JOIN game_metadata m ON m.game_id = g.id
  LEFT JOIN artwork ac ON ac.game_id = g.id AND ac.kind = 'cover' AND ac.status = 'cached'
  LEFT JOIN artwork ah ON ah.game_id = g.id AND ah.kind = 'hero' AND ah.status = 'cached'`;

/** Sort/search key: a renamed game sorts and matches under its new title. */
const TITLE_KEY = "COALESCE(m.title_override_search, g.title_search)";

/** "Recently played" filter window: 30 days (`RECENTLY_PLAYED_WINDOW_S`). */
export const RECENTLY_PLAYED_WINDOW_S = 30 * 24 * 60 * 60;

/** `filter_condition`; `?5` (window) is bound only for `recently-played`. */
const FILTER_CONDITION: Record<GameFilter, string> = {
  all: "",
  "recently-played": "AND s.last_played_at >= unixepoch() - ?5",
  unplayed: "AND s.game_id IS NULL",
  "needs-review": "AND g.dat_name IS NULL",
  favorites: "AND g.favorite = 1",
  missing: "AND g.missing = 1",
  played: "AND s.last_played_at IS NOT NULL",
};

/** `order_by` (always ends on the title key, then `g.id`). */
const ORDER_BY: Record<GameSort, string> = {
  "recently-played": `s.last_played_at DESC NULLS LAST, ${TITLE_KEY}`,
  title: TITLE_KEY,
  "play-time": `COALESCE(s.total_play_s, 0) DESC, ${TITLE_KEY}`,
  "recently-added": `g.added_at DESC, ${TITLE_KEY}`,
};

/** `list_games_sql`: visible games (rows of removed folders are hidden). Binds `?1`
 *  search pattern or NULL, `?2` genre, `?3` platform (NULL = any), `?4` limit (NULL =
 *  all), `?5` window (recently-played only). */
export function listGamesSql(filter: GameFilter, sort: GameSort): string {
  return `${GAME_SELECT}
    WHERE g.folder_id IS NOT NULL
      AND (?1 IS NULL OR ${TITLE_KEY} LIKE ?1 ESCAPE '\\')
      AND (?2 IS NULL OR m.genre = ?2)
      AND (?3 IS NULL OR g.platform_id = ?3)
      ${FILTER_CONDITION[filter]}
    ORDER BY ${ORDER_BY[sort]}, g.id
    LIMIT COALESCE(?4, -1)`;
}

/** `search_pattern`: `LIKE` pattern of a search box value — lower-cased, wildcards
 *  escaped; blank → null. */
export function searchPattern(search: string | undefined): string | null {
  const trimmed = search?.trim();
  if (!trimmed) return null;
  let pattern = "%";
  for (const ch of trimmed.toLowerCase()) {
    if (ch === "%" || ch === "_" || ch === "\\") pattern += "\\";
    pattern += ch;
  }
  return `${pattern}%`;
}
