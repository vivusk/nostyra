// Edit metadata dialog values → stored values, mirroring
// crates/storage-db/src/user_metadata_edits.rs: trimmed, length-capped, range-checked
// (out of range = an `invalid` error with the desktop's message).
import type { UserMetadataEdit } from "../../../api/tauri-commands-artwork";

const MAX_TITLE_CHARS = 200;
const MAX_FIELD_CHARS = 200;
const MAX_DESCRIPTION_CHARS = 4_000;
const YEAR_RANGE = [1980, 2030] as const;
const PLAYERS_RANGE = [1, 8] as const;

export interface CleanUserMetadata {
  title: string | null;
  titleSearch: string | null;
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  releaseYear: number | null;
  players: number | null;
  description: string | null;
}

/** Thrown for a value the desktop would reject as `InvalidInput`. */
export class InvalidMetadataInput extends Error {}

/** `clean_text`: trimmed value capped at `max` characters; blank → null. */
function cleanText(value: string | undefined | null, max: number): string | null {
  const trimmed = value?.trim();
  return trimmed ? [...trimmed].slice(0, max).join("") : null;
}

/** `checked_number`. */
function checkedNumber(value: number | undefined | null, [min, max]: readonly [number, number], field: string): number | null {
  if (value === undefined || value === null) return null;
  if (value < min || value > max) throw new InvalidMetadataInput(`${field} must be between ${min} and ${max}`);
  return value;
}

/** The row values `update_user_metadata` writes. */
export function cleanUserMetadata(edit: UserMetadataEdit): CleanUserMetadata {
  const title = cleanText(edit.title, MAX_TITLE_CHARS);
  return {
    title,
    titleSearch: title === null ? null : title.toLowerCase(),
    developer: cleanText(edit.developer, MAX_FIELD_CHARS),
    publisher: cleanText(edit.publisher, MAX_FIELD_CHARS),
    genre: cleanText(edit.genre, MAX_FIELD_CHARS),
    releaseYear: checkedNumber(edit.releaseYear, YEAR_RANGE, "year"),
    players: checkedNumber(edit.players, PLAYERS_RANGE, "players"),
    description: cleanText(edit.description, MAX_DESCRIPTION_CHARS),
  };
}
