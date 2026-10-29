// Edit metadata form state: text fields seeded from the game, validated into a
// `UserMetadataEdit` (the backend validates and caps again).
import { useState } from "react";
import type { GameDetail, UserMetadataEdit } from "../api/tauri-commands-artwork";

export interface MetadataFormValues {
  title: string;
  developer: string;
  publisher: string;
  genre: string;
  releaseYear: string;
  players: string;
  description: string;
}

export const YEAR_MIN = 1980;
export const YEAR_MAX = 2030;
export const PLAYERS_MAX = 8;
export const TITLE_MAX = 200;
/** Developer / publisher / genre (backend caps at the same length). */
export const FIELD_MAX = 200;
export const DESCRIPTION_MAX = 4000;

function formValuesFrom(game: GameDetail): MetadataFormValues {
  const meta = game.metadata;
  return {
    title: game.title,
    developer: meta?.developer ?? "",
    publisher: meta?.publisher ?? "",
    genre: meta?.genre ?? "",
    releaseYear: meta?.releaseYear?.toString() ?? "",
    players: meta?.players?.toString() ?? "",
    description: meta?.description ?? "",
  };
}

/** Whole number in range, blank = undefined; `null` = invalid. */
function parseRange(text: string, min: number, max: number): number | undefined | null {
  const trimmed = text.trim();
  if (!trimmed) return undefined;
  const value = Number(trimmed);
  return Number.isInteger(value) && value >= min && value <= max ? value : null;
}

/** The edit to save, or a readable validation error. The title is only sent when it
 *  differs from the scanned name or an override already exists (no needless override). */
export function toUserEdit(game: GameDetail, values: MetadataFormValues): { edit?: UserMetadataEdit; error?: string } {
  const releaseYear = parseRange(values.releaseYear, YEAR_MIN, YEAR_MAX);
  if (releaseYear === null) return { error: `Year must be between ${YEAR_MIN} and ${YEAR_MAX}.` };
  const players = parseRange(values.players, 1, PLAYERS_MAX);
  if (players === null) return { error: `Players must be between 1 and ${PLAYERS_MAX}.` };
  const titleChanged = values.title.trim() !== game.title || Boolean(game.metadata?.titleOverride);
  return {
    edit: {
      title: titleChanged ? values.title : undefined,
      developer: values.developer,
      publisher: values.publisher,
      genre: values.genre,
      releaseYear,
      players,
      description: values.description,
    },
  };
}

export function useEditMetadataForm(game: GameDetail) {
  const [values, setValues] = useState<MetadataFormValues>(() => formValuesFrom(game));
  const setField = (field: keyof MetadataFormValues, value: string) =>
    setValues((current) => ({ ...current, [field]: value }));
  return { values, setField };
}
