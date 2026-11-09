import { useState } from "react";
import { useOpenUpdateReleasePage, useSkipUpdateVersion, useUpdateStatus } from "../hooks/use-update-queries";
import { bannerTitle, shouldShowUpdateBanner } from "../lib/update-banner-rule";

const BUTTON =
  "h-11 cursor-pointer border-0 bg-transparent font-body text-sm font-bold text-link disabled:cursor-default disabled:opacity-60";

/** Desktop: a newer release with the portable zip exists. Skip hides that version
 *  for good; Later hides the banner until the launcher restarts. The query is off
 *  on the web edition, so nothing renders there. */
export function UpdateAvailableBanner() {
  const { data: status } = useUpdateStatus();
  const skip = useSkipUpdateVersion();
  const openPage = useOpenUpdateReleasePage();
  const [later, setLater] = useState(false);
  if (!shouldShowUpdateBanner(status, later) || !status?.latest) return null;
  const { version, excerpt } = status.latest;
  const title = bannerTitle(status.latest.title, version);

  return (
    <div className="mb-6 flex flex-col gap-1 rounded-card border border-accent/60 bg-surface px-[18px] py-3 text-sm text-text-soft">
      <div role="status">
        <p className="m-0">
          Nostyra <strong className="text-white">{version}</strong> is available
          {title ? ` — ${title}` : ""}
        </p>
        {/* Plain text from the backend (Markdown already stripped); never HTML. */}
        {excerpt && <p className="m-0 mt-1 line-clamp-2 text-xs text-muted">{excerpt}</p>}
      </div>
      <div className="flex flex-wrap gap-5">
        <button type="button" onClick={() => openPage.mutate()} disabled={openPage.isPending} className={BUTTON}>
          Open release page
        </button>
        <button type="button" onClick={() => skip.mutate(version)} disabled={skip.isPending} className={BUTTON}>
          Skip this version
        </button>
        <button type="button" onClick={() => setLater(true)} className={BUTTON}>
          Later
        </button>
      </div>
    </div>
  );
}
