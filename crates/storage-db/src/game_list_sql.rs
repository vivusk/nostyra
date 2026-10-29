//! SQL fragments of the library list query. Filter/sort fragments come from fixed
//! enums, never from user input; search, genre, platform, window and limit are bound
//! by name.

use crate::domain_types::{GameFilter, GameSort};

/// Game columns + play stats + metadata summary + cached art, in [`crate::Game`] order.
pub(crate) const GAME_SELECT: &str = "SELECT g.id, g.platform_id, g.folder_id, g.rom_path,
        g.file_size, g.crc32, g.sha1, COALESCE(m.title_override, g.title), g.region,
        g.dat_name, g.added_at, g.missing, COALESCE(s.total_play_s, 0), s.last_played_at,
        m.genre, m.release_year, m.developer, g.favorite,
        ac.local_path, ah.local_path, ac.checked_at, g.serial
    FROM games g
    LEFT JOIN game_stats s ON s.game_id = g.id
    LEFT JOIN game_metadata m ON m.game_id = g.id
    LEFT JOIN artwork ac ON ac.game_id = g.id AND ac.kind = 'cover' AND ac.status = 'cached'
    LEFT JOIN artwork ah ON ah.game_id = g.id AND ah.kind = 'hero' AND ah.status = 'cached'";

/// Sort/search key: a renamed game sorts and matches under its new title.
const TITLE_KEY: &str = "COALESCE(m.title_override_search, g.title_search)";

/// Extra `WHERE` condition of a filter; `:window` is bound only for `RecentlyPlayed`.
pub(crate) fn filter_condition(filter: GameFilter) -> &'static str {
    match filter {
        GameFilter::All => "",
        GameFilter::RecentlyPlayed => "AND s.last_played_at >= unixepoch() - :window",
        GameFilter::Unplayed => "AND s.game_id IS NULL",
        GameFilter::NeedsReview => "AND g.dat_name IS NULL",
        GameFilter::Favorites => "AND g.favorite = 1",
        GameFilter::Missing => "AND g.missing = 1",
        GameFilter::Played => "AND s.last_played_at IS NOT NULL",
    }
}

/// `ORDER BY` list of a sort (always ends on the title key; caller appends `g.id`).
fn order_by(sort: GameSort) -> String {
    match sort {
        GameSort::RecentlyPlayed => format!("s.last_played_at DESC NULLS LAST, {TITLE_KEY}"),
        GameSort::Title => TITLE_KEY.to_string(),
        GameSort::PlayTime => format!("COALESCE(s.total_play_s, 0) DESC, {TITLE_KEY}"),
        GameSort::RecentlyAdded => format!("g.added_at DESC, {TITLE_KEY}"),
    }
}

/// Full list statement: visible games (rows of removed folders are hidden).
/// Binds `:search` (LIKE pattern or NULL), `:genre` and `:platform` (or NULL = any),
/// `:limit` (or NULL = all).
pub(crate) fn list_games_sql(filter: GameFilter, sort: GameSort) -> String {
    format!(
        "{GAME_SELECT}
         WHERE g.folder_id IS NOT NULL
           AND (:search IS NULL OR {TITLE_KEY} LIKE :search ESCAPE '\\')
           AND (:genre IS NULL OR m.genre = :genre)
           AND (:platform IS NULL OR g.platform_id = :platform)
           {filter}
         ORDER BY {order}, g.id
         LIMIT COALESCE(:limit, -1)",
        filter = filter_condition(filter),
        order = order_by(sort),
    )
}
