// Pure rule for the update banner (import-free so `node --test` can load it).

/** The status fields the rule reads (structurally matches `UpdateStatus`). */
export interface UpdateBannerInput {
  updateAvailable: boolean;
  latest: { version: string } | null;
  skippedVersion: string | null;
}

/** Banner shows for an available, not skipped update until "Later" is pressed. */
export function shouldShowUpdateBanner(status: UpdateBannerInput | null | undefined, later: boolean): boolean {
  if (!status || later || !status.updateAvailable || !status.latest) return false;
  return status.latest.version !== status.skippedVersion;
}

/** Release title without a leading "Nostyra <version>" (optional `v`) and the separator
 *  after it (" — ", " – ", ": ", " - "), since the banner already names the version.
 *  "Nostyra 0.4.0 — PlayStation 2" → "PlayStation 2"; "Nostyra 0.4.0" → "". */
export function bannerTitle(title: string, version: string): string {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const prefix = new RegExp(`^\\s*Nostyra\\s+v?${escaped}(?![\\w.-])\\s*(?:[—–:-]\\s*)?`, "i");
  return title.replace(prefix, "").trim();
}
