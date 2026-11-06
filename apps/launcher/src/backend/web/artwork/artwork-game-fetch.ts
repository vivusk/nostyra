// One game's art (crates/artwork-cache game_artwork_fetcher.rs on the web): name
// candidates → index lookup (or direct GET) → download from the GitHub raw mirror →
// validate / re-encode → OPFS, reported per kind so the job stores cached /
// not-found / failed and skips transient results (retried by the next job). The
// first transient or cancelled kind ends the game.
import type { ArtworkKind } from "../db/artwork-queries";
import type { ThumbnailIndex } from "./artwork-index";
import { writeArtworkFile } from "./artwork-store";
import { InvalidImage, encodeCoverJpeg, validateHeroPng } from "./image-resize";
import { resolveNames, thumbnailUrl, type ThumbnailFolder } from "./thumbnail-names";

/** Per-request time limit. */
const REQUEST_TIMEOUT_MS = 20_000;

export type KindOutcome =
  | { kind: "cached"; localPath: string; sourceName: string }
  | { kind: "not-found" }
  | { kind: "failed"; reason: string }
  | { kind: "transient"; reason: string }
  | { kind: "cancelled" };

export interface GameArtRequest {
  gameId: number;
  repo: string;
  noIntroName: string;
  /** Shown in progress. */
  title: string;
  kinds: ArtworkKind[];
}

export interface ArtworkOutcome {
  results: [ArtworkKind, KindOutcome][];
  /** At least one request completed a network round-trip (any HTTP answer). */
  network: boolean;
}

/** `ArtworkKind::folders`: thumbnail folders to try, best first. */
const FOLDERS: Record<ArtworkKind, ThumbnailFolder[]> = {
  cover: ["Named_Boxarts"],
  hero: ["Named_Snaps", "Named_Titles"],
};

/** `ArtworkKind::file_name`. */
export function artworkFileName(kind: ArtworkKind, gameId: number): string {
  return kind === "cover" ? `${gameId}-cover.jpg` : `${gameId}-hero.png`;
}

export async function fetchGameArtwork(
  request: GameArtRequest,
  index: ThumbnailIndex | null,
  signal: AbortSignal,
): Promise<ArtworkOutcome> {
  const candidates = resolveNames(request.noIntroName);
  const outcome: ArtworkOutcome = { results: [], network: false };
  let stop: KindOutcome | null = null;
  for (const kind of new Set(request.kinds)) {
    const result: KindOutcome = stop ?? (await fetchKind(request, kind, candidates, index, signal, outcome));
    if (result.kind === "transient" || result.kind === "cancelled") stop ??= result;
    outcome.results.push([kind, result]);
  }
  return outcome;
}

async function fetchKind(
  request: GameArtRequest,
  kind: ArtworkKind,
  candidates: string[],
  index: ThumbnailIndex | null,
  signal: AbortSignal,
  outcome: ArtworkOutcome,
): Promise<KindOutcome> {
  for (const folder of FOLDERS[kind]) {
    const names = index ? [index.findName(folder, candidates)].filter((n): n is string => n !== null) : candidates;
    for (const name of names) {
      if (signal.aborted) return { kind: "cancelled" };
      let response: Response;
      try {
        response = await fetch(thumbnailUrl(request.repo, folder, name), {
          signal: AbortSignal.any([signal, AbortSignal.timeout(REQUEST_TIMEOUT_MS)]),
        });
      } catch (error) {
        if (signal.aborted) return { kind: "cancelled" };
        return { kind: "transient", reason: error instanceof Error ? error.message : String(error) };
      }
      outcome.network = true;
      // Index lag or a direct-GET miss: try the next name / folder.
      if (response.status === 404) continue;
      if (response.status === 403 || response.status === 429 || response.status >= 500) {
        return { kind: "transient", reason: `HTTP ${response.status}` };
      }
      if (!response.ok) return { kind: "failed", reason: `HTTP ${response.status}` };
      let body: Blob;
      try {
        body = await response.blob();
      } catch (error) {
        if (signal.aborted) return { kind: "cancelled" };
        return { kind: "transient", reason: error instanceof Error ? error.message : String(error) };
      }
      return store(kind, request.gameId, folder, name, body);
    }
  }
  return { kind: "not-found" };
}

async function store(kind: ArtworkKind, gameId: number, folder: ThumbnailFolder, name: string, blob: Blob): Promise<KindOutcome> {
  const localPath = artworkFileName(kind, gameId);
  try {
    const bytes = kind === "cover" ? await encodeCoverJpeg(blob) : await validateHeroPng(blob);
    await writeArtworkFile(localPath, bytes);
    return { kind: "cached", localPath, sourceName: `${folder}/${name}.png` };
  } catch (error) {
    const reason = error instanceof InvalidImage || error instanceof Error ? error.message : String(error);
    return { kind: "failed", reason };
  }
}
