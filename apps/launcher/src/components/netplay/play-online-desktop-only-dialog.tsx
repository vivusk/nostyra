import { useEffect, useRef } from "react";
import { DESKTOP_DOWNLOAD_URL } from "../../lib/netplay-rules";

const BUTTON = "inline-flex h-11 cursor-pointer items-center rounded-control px-5 font-body font-bold";

/** Web edition: Play online exists only in the desktop app. Explains it and links to
 *  the download page (new tab, so the launcher page and its folder access stay open). */
export function PlayOnlineDesktopOnlyDialog({ onClose }: { onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => dialogRef.current?.showModal(), []);

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="play-online-desktop-only-title"
      className="m-auto w-[min(440px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="play-online-desktop-only-title" className="m-0 font-display text-[22px] font-semibold">
          Play online
        </h2>
        <p className="m-0 text-sm text-text-soft">
          Online co-op is available in the Nostyra desktop app only. Download Nostyra for Windows to host a room or
          join your friends with a room code.
        </p>
        <div className="flex justify-end gap-3">
          <button type="button" onClick={onClose} className={`${BUTTON} border-0 bg-transparent text-text-soft hover:text-white`}>
            Close
          </button>
          <a
            href={DESKTOP_DOWNLOAD_URL}
            target="_blank"
            rel="noreferrer"
            autoFocus
            onClick={onClose}
            className={`${BUTTON} bg-accent-strong text-white no-underline hover:bg-accent`}
          >
            Download the desktop app
          </a>
        </div>
      </div>
    </dialog>
  );
}
