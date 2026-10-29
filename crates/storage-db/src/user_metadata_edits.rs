//! User edits from the Edit metadata dialog: trimmed, length-capped, range-checked,
//! then saved as a user-edited row (frozen against index refreshes until reset).

use rusqlite::{named_params, Connection};

use crate::game_scan_writes::title_search_key;
use crate::metadata_and_artwork_types::UserMetadataEdit;
use crate::storage_error::{StorageError, StorageResult};

/// Length caps of user-edited fields (characters).
const MAX_TITLE_CHARS: usize = 200;
const MAX_FIELD_CHARS: usize = 200;
const MAX_DESCRIPTION_CHARS: usize = 4_000;
/// Accepted ranges of user-edited numbers.
const YEAR_RANGE: std::ops::RangeInclusive<i64> = 1980..=2030;
const PLAYERS_RANGE: std::ops::RangeInclusive<i64> = 1..=8;

/// Saves the user's values (trimmed, capped, validated) and marks the row user-edited,
/// which freezes all fields against index refreshes until [`crate::delete_metadata`] (reset).
pub fn update_user_metadata(
    conn: &Connection,
    game_id: i64,
    edit: &UserMetadataEdit,
    source_version: &str,
    now: i64,
) -> StorageResult<()> {
    let title = clean_text(edit.title.as_deref(), MAX_TITLE_CHARS);
    let title_search = title.as_deref().map(title_search_key);
    let year = checked_number(edit.release_year, &YEAR_RANGE, "year")?;
    let players = checked_number(edit.players, &PLAYERS_RANGE, "players")?;
    conn.execute(
        "INSERT INTO game_metadata (game_id, title_override, title_override_search, developer,
            publisher, genre, release_year, players, description, source_version, user_edited,
            updated_at)
         VALUES (:id, :title, :title_search, :developer, :publisher, :genre, :year, :players,
            :description, :version, 1, :now)
         ON CONFLICT(game_id) DO UPDATE SET title_override = excluded.title_override,
            title_override_search = excluded.title_override_search,
            developer = excluded.developer, publisher = excluded.publisher,
            genre = excluded.genre, release_year = excluded.release_year,
            players = excluded.players, description = excluded.description,
            user_edited = 1, updated_at = excluded.updated_at",
        named_params! {
            ":id": game_id,
            ":title": title,
            ":title_search": title_search,
            ":developer": clean_text(edit.developer.as_deref(), MAX_FIELD_CHARS),
            ":publisher": clean_text(edit.publisher.as_deref(), MAX_FIELD_CHARS),
            ":genre": clean_text(edit.genre.as_deref(), MAX_FIELD_CHARS),
            ":year": year,
            ":players": players,
            ":description": clean_text(edit.description.as_deref(), MAX_DESCRIPTION_CHARS),
            ":version": source_version,
            ":now": now,
        },
    )?;
    Ok(())
}

/// Trimmed value capped at `max` characters; blank → `None`.
fn clean_text(value: Option<&str>, max: usize) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.chars().take(max).collect())
}

fn checked_number(
    value: Option<i64>,
    range: &std::ops::RangeInclusive<i64>,
    field: &str,
) -> StorageResult<Option<i64>> {
    match value {
        Some(n) if !range.contains(&n) => Err(StorageError::InvalidInput(format!(
            "{field} must be between {} and {}",
            range.start(),
            range.end()
        ))),
        other => Ok(other),
    }
}
