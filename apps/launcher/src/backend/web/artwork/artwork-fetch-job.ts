// The web artwork job (src-tauri artwork_fetch_job.rs + artwork_job_accounting.rs):
// one at a time, 4 games in parallel; a request while it runs is queued (rerun flag
// / game list) and served by another pass. Stores cached / not-found / failed rows,
// never transient ones; stops itself after 5 consecutive all-transient games (dead
// network). Emits `artwork-progress` / `artwork-updated` / `artwork-finished` with
// the desktop payloads.
import type { ArtworkFinishedEvent, ArtworkProgressEvent } from "../../../api/tauri-events";
import { upsertArtwork } from "../db/artwork-queries";
import { emit } from "../web-event-bus";
import { loadThumbnailIndex, type ThumbnailIndex } from "./artwork-index";
import { fetchGameArtwork, type ArtworkOutcome, type GameArtRequest } from "./artwork-game-fetch";
import { gameArtworkRequests, missingArtworkRequests } from "./artwork-work-list";

const ARTWORK_WORKERS = 4;
/** Consecutive all-transient games after which the job stops (`ARTWORK_TRANSIENT_BREAK`). */
const TRANSIENT_BREAK = 5;
const THUMBNAIL_SOURCE = "libretro-thumbnails";

let controller: AbortController | null = null;
let rerunMissing = false;
let queuedGames: number[] = [];
let progress: ArtworkProgressEvent | null = null;

export const isArtworkFetching = () => controller !== null;
export const currentArtworkProgress = () => progress;

function setProgress(next: ArtworkProgressEvent): void {
  progress = next;
  emit("artwork-progress", next);
}

/** `start_artwork_job`: all missing art (`null`) or one game; queued while running. */
export function startArtworkFetch(gameId: number | null): void {
  if (controller) {
    if (gameId === null) rerunMissing = true;
    else queuedGames.push(gameId);
    return;
  }
  controller = new AbortController();
  setProgress({ done: 0, total: 0, currentTitle: null });
  void runJob(controller, gameId);
}

export function cancelArtworkFetch(): void {
  controller?.abort();
}

async function runJob(job: AbortController, firstGame: number | null): Promise<void> {
  const totals: ArtworkFinishedEvent = { cached: 0, notFound: 0, failed: 0, transient: 0, cancelled: false, error: null };
  const errors: string[] = [];
  const breaker = { consecutive: 0, tripped: false };
  let missing = firstGame === null;
  let games = firstGame === null ? [] : [firstGame];
  for (;;) {
    try {
      await runPass(job, breaker, missing, games, totals);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
    if (job.signal.aborted) break;
    missing = rerunMissing;
    games = queuedGames;
    rerunMissing = false;
    queuedGames = [];
    if (!missing && games.length === 0) break;
  }
  rerunMissing = false;
  queuedGames = [];
  controller = null;
  progress = null;
  totals.cancelled = job.signal.aborted && !breaker.tripped;
  totals.error = errors.length > 0 ? errors.join("\n") : null;
  emit("artwork-finished", totals);
}

async function runPass(
  job: AbortController,
  breaker: { consecutive: number; tripped: boolean },
  missing: boolean,
  games: number[],
  totals: ArtworkFinishedEvent,
): Promise<void> {
  const work = missing ? await missingArtworkRequests(Math.floor(Date.now() / 1000)) : [];
  for (const request of await gameArtworkRequests(games)) {
    if (work.every((w) => w.gameId !== request.gameId)) work.push(request);
  }
  if (work.length === 0) return;
  const indexes = new Map<string, Promise<ThumbnailIndex | null>>();
  let next = 0;
  let done = 0;
  const worker = async () => {
    while (next < work.length && !job.signal.aborted) {
      const request = work[next++];
      let index = indexes.get(request.repo);
      if (!index) indexes.set(request.repo, (index = loadThumbnailIndex(request.repo)));
      const outcome = await fetchGameArtwork(request, await index, job.signal);
      if (recordBreaker(breaker, outcome)) job.abort();
      count(totals, outcome);
      if (await storeOutcome(request, outcome)) emit("artwork-updated", { gameId: request.gameId });
      setProgress({ done: ++done, total: work.length, currentTitle: request.title });
    }
  };
  await Promise.all(Array.from({ length: ARTWORK_WORKERS }, worker));
}

/** `TransientBreaker::record`: true when this game trips the breaker. Only an
 *  outcome with a network round-trip resets the streak. */
function recordBreaker(breaker: { consecutive: number; tripped: boolean }, outcome: ArtworkOutcome): boolean {
  const transient = outcome.results.length > 0 && outcome.results.every(([, r]) => r.kind === "transient");
  if (transient) {
    breaker.consecutive += 1;
    if (breaker.consecutive >= TRANSIENT_BREAK && !breaker.tripped) return (breaker.tripped = true);
    return false;
  }
  if (outcome.network) breaker.consecutive = 0;
  return false;
}

/** `count_outcome`. */
function count(totals: ArtworkFinishedEvent, outcome: ArtworkOutcome): void {
  for (const [, result] of outcome.results) {
    if (result.kind === "cached") totals.cached += 1;
    else if (result.kind === "not-found") totals.notFound += 1;
    else if (result.kind === "failed") totals.failed += 1;
    else if (result.kind === "transient") totals.transient += 1;
  }
}

/** Writes the decided kinds (`artwork_record`); returns whether any row changed. */
async function storeOutcome(request: GameArtRequest, outcome: ArtworkOutcome): Promise<boolean> {
  let wrote = false;
  const checkedAt = Math.floor(Date.now() / 1000);
  for (const [kind, result] of outcome.results) {
    if (result.kind === "transient" || result.kind === "cancelled") continue;
    const cached = result.kind === "cached" ? result : null;
    try {
      await upsertArtwork({
        gameId: request.gameId,
        kind,
        status: result.kind,
        localPath: cached?.localPath ?? null,
        source: THUMBNAIL_SOURCE,
        sourceName: cached?.sourceName ?? null,
        checkedAt,
      });
      wrote = true;
    } catch (error) {
      console.warn(`artwork row not saved (game ${request.gameId})`, error);
    }
  }
  return wrote;
}
