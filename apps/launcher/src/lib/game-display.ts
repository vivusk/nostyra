// Presentation helpers for game cards, Home and Detail (monogram tiles when no art).

/** Muted tile colors from the approved mockup. */
const TILE_COLORS = [
  "#2A3570", "#5A2A2A", "#2C4A28", "#5A4A1A", "#4A3A1A", "#4A2A4A",
  "#1E4A45", "#2A4A3A", "#1A3A5A", "#2A2A4A", "#3A2A1A",
];

/** Same title → same color on every launch (FNV-1a hash). */
export function tileColor(title: string): string {
  let hash = 0x811c9dc5;
  for (const ch of title) {
    hash ^= ch.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193);
  }
  return TILE_COLORS[(hash >>> 0) % TILE_COLORS.length];
}

/** Up to 3 initials ("Super Mario World" → "SMW"); "?" when nothing usable. */
export function monogram(title: string): string {
  const initials = title
    .split(/[\s\-_:.]+/)
    .map((word) => [...word].find((ch) => /[\p{L}\p{N}]/u.test(ch)) ?? "")
    .filter(Boolean)
    .slice(0, 3)
    .join("")
    .toUpperCase();
  return initials || "?";
}

/** "41h", "1h 20m", "35m", "<1m". */
export function formatPlayTime(seconds: number): string {
  if (seconds < 60) return "<1m";
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return `${minutes}m`;
  const rest = minutes % 60;
  return hours >= 10 || rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}

/** Card meta line: "USA · 12h" / "USA · Not played". */
export function gameMetaLine(region: string | null, totalPlayS: number, lastPlayedAt: number | null): string {
  const played = lastPlayedAt === null ? "Not played" : formatPlayTime(totalPlayS);
  return [region, played].filter(Boolean).join(" · ");
}

/** Home hero / card meta: "SNES · 1995 · RPG" (missing parts skipped); `systemLabel`
 *  is the game's system short label. */
export function homeMetaLine(
  systemLabel: string,
  game: { releaseYear: number | null; genre: string | null },
): string {
  return [systemLabel, game.releaseYear, game.genre].filter(Boolean).join(" · ");
}

/** Detail meta: "Square · 1995 · RPG · 2 players" (missing parts skipped). */
export function detailMetaLine(parts: {
  developer: string | null;
  releaseYear: number | null;
  genre: string | null;
  players: number | null;
}): string {
  const players = parts.players === null ? null : parts.players === 1 ? "1 player" : `${parts.players} players`;
  return [parts.developer, parts.releaseYear, parts.genre, players].filter(Boolean).join(" · ");
}

/** "Oct 18, 2026, 13:52" in the user's locale. */
export function formatDateTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
