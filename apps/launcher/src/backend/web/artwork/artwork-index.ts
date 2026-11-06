// Which thumbnail files exist upstream, from one GitHub tree API response per system
// (crates/artwork-cache thumbnail_index*.rs), so the normal path never probes with
// 404s. The web caches the parsed names in OPFS for 7 days (the API allows 60
// unauthenticated requests per hour per visitor) and revalidates with the ETag; at
// most one network attempt per system per page. Failure → stale cache, else null
// (the caller then GETs candidate names directly).
import { readIndexFile, writeIndexFile } from "./artwork-store";
import { indexUrl, type ThumbnailFolder } from "./thumbnail-names";

/** Age under which the cached index is used without asking GitHub. */
const INDEX_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;
const FOLDERS: ThumbnailFolder[] = ["Named_Boxarts", "Named_Snaps", "Named_Titles"];

interface CachedIndex {
  fetchedAt: number;
  etag: string | null;
  names: Record<ThumbnailFolder, string[]>;
}

export class ThumbnailIndex {
  private readonly sets: Record<ThumbnailFolder, Set<string>>;

  constructor(names: Record<ThumbnailFolder, string[]>) {
    this.sets = {
      Named_Boxarts: new Set(names.Named_Boxarts),
      Named_Snaps: new Set(names.Named_Snaps),
      Named_Titles: new Set(names.Named_Titles),
    };
  }

  /** `find_name`: first of `candidates` present in `folder`. */
  findName(folder: ThumbnailFolder, candidates: string[]): string | null {
    return candidates.find((name) => this.sets[folder].has(name)) ?? null;
  }
}

/** `parse_tree_json`: file names (without `.png`) per folder; nested files ignored; an
 *  index without box art is rejected (it would mark every game "not found"). */
export function parseTree(json: unknown): Record<ThumbnailFolder, string[]> {
  const tree = (json as { tree?: { path?: string; type?: string }[] }).tree ?? [];
  const names: Record<ThumbnailFolder, string[]> = { Named_Boxarts: [], Named_Snaps: [], Named_Titles: [] };
  for (const entry of tree) {
    if (entry.type !== "blob" || !entry.path) continue;
    const slash = entry.path.indexOf("/");
    const dir = entry.path.slice(0, slash) as ThumbnailFolder;
    const file = entry.path.slice(slash + 1);
    if (slash < 0 || !FOLDERS.includes(dir) || !file.endsWith(".png") || file.includes("/")) continue;
    names[dir].push(file.slice(0, -".png".length));
  }
  if (names.Named_Boxarts.length === 0) throw new Error("thumbnail index has no Named_Boxarts entries");
  return names;
}

const attempted = new Set<string>();

/** A thumbnail repo's index (cache, else one conditional GET per page), or null. One
 *  cache file per repo: `index/<repo>-tree.json`. */
export async function loadThumbnailIndex(repo: string): Promise<ThumbnailIndex | null> {
  const file = `${repo}-tree.json`;
  const cached = parseCached(await readIndexFile(file));
  if (cached && Date.now() - cached.fetchedAt < INDEX_MAX_AGE_MS) return new ThumbnailIndex(cached.names);
  if (!attempted.has(repo)) {
    attempted.add(repo);
    try {
      const headers: Record<string, string> = { Accept: "application/vnd.github+json" };
      if (cached?.etag) headers["If-None-Match"] = cached.etag;
      const response = await fetch(indexUrl(repo), { headers, cache: "no-store" });
      if (response.status === 304 && cached) {
        await writeIndexFile(file, JSON.stringify({ ...cached, fetchedAt: Date.now() }));
        return new ThumbnailIndex(cached.names);
      }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const names = parseTree(await response.json());
      const fresh: CachedIndex = { fetchedAt: Date.now(), etag: response.headers.get("etag"), names };
      await writeIndexFile(file, JSON.stringify(fresh));
      return new ThumbnailIndex(names);
    } catch (error) {
      console.warn(`thumbnail index refresh failed (${repo})`, error);
    }
  }
  return cached ? new ThumbnailIndex(cached.names) : null;
}

function parseCached(text: string | null): CachedIndex | null {
  if (!text) return null;
  try {
    return JSON.parse(text) as CachedIndex;
  } catch {
    return null;
  }
}
