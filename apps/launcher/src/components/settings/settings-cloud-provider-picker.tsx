import { useState } from "react";
import { open } from "../../backend";
import { errorMessage } from "../../api/tauri-commands";
import { cloudCommands, type CloudProviderId, type CloudStatus } from "../../api/tauri-commands-cloud";
import { useCloudSignIn, useCloudSignOut, useSetCloudLocalFolder } from "../../hooks/use-cloud-queries";
import { ConfirmDialog } from "../confirm-dialog";

export const CLOUD_PROVIDER_NAMES: Record<CloudProviderId, string> = {
  off: "Off",
  google_drive: "Google Drive",
  onedrive: "OneDrive",
  dropbox: "Dropbox",
  local_folder: "Local folder",
};

const ORDER: CloudProviderId[] = ["off", "google_drive", "onedrive", "dropbox", "local_folder"];

export const CLOUD_BUTTON =
  "h-11 w-fit cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50";
const PRIMARY =
  "h-11 w-fit cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50";

/** Provider select + what the choice needs: the folder picker (Local folder) or the
 *  browser sign-in with its waiting state and Cancel (Google Drive / OneDrive /
 *  Dropbox; "not available in this version yet" until their providers ship).
 *  Locked while signed in: Sign out first to switch. */
export function SettingsCloudProviderPicker({ status }: { status: CloudStatus }) {
  const [choice, setChoice] = useState<CloudProviderId>(status.provider);
  const locked = status.signedIn;
  const selected = locked ? status.provider : choice;
  // "Sign in again" banner already offers the fix for the configured provider.
  const showSetup = !locked && !(status.needsSignIn && selected === status.provider);

  return (
    <div className="flex flex-col gap-3">
      <label className="flex flex-wrap items-center gap-3 text-sm text-text-soft">
        Sync saves with
        <select
          value={selected}
          disabled={locked}
          onChange={(e) => setChoice(e.target.value as CloudProviderId)}
          className="h-11 rounded-control border border-border-strong bg-surface-raised px-3 font-body text-sm text-text disabled:opacity-60"
        >
          {ORDER.map((id) => (
            <option key={id} value={id}>
              {CLOUD_PROVIDER_NAMES[id]}
            </option>
          ))}
        </select>
      </label>
      {locked && <p className="m-0 text-xs text-muted">Sign out to choose another provider.</p>}
      {showSetup && selected === "local_folder" && <LocalFolderPicker current={status.localFolder} />}
      {showSetup && selected !== "off" && selected !== "local_folder" && <SignInButton provider={selected} />}
      {!locked && selected === "off" && status.provider !== "off" && <TurnOffButton status={status} />}
    </div>
  );
}

/** "Off" chosen while a provider is still configured but not signed in (needs sign-in):
 *  clears it like Sign out, so the "Cloud saves paused" banner goes away. */
function TurnOffButton({ status }: { status: CloudStatus }) {
  const signOut = useCloudSignOut();
  const [confirm, setConfirm] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setConfirm(true)} disabled={signOut.isPending} className={CLOUD_BUTTON}>
        Turn off cloud saves
      </button>
      {confirm && (
        <ConfirmDialog
          title={`Stop syncing with ${CLOUD_PROVIDER_NAMES[status.provider]}?`}
          confirmLabel="Turn off"
          onConfirm={() => signOut.mutate()}
          onClose={() => setConfirm(false)}
        >
          Saves stay in your cloud and on this PC. Cloud saves stop syncing until you sign in again.
        </ConfirmDialog>
      )}
    </>
  );
}

/** Local folder: current folder + picker; the backend checks it (exists, writable,
 *  outside the data folder) and says why it refuses. */
export function LocalFolderPicker({ current, label = "Choose folder…" }: { current: string; label?: string }) {
  const setFolder = useSetCloudLocalFolder();
  const pick = async () => {
    const path = await open({ directory: true, multiple: false, title: "Choose the cloud saves folder", defaultPath: current || undefined });
    if (typeof path === "string") setFolder.mutate(path);
  };
  return (
    <div className="flex flex-col gap-2">
      {current && <p className="m-0 break-all text-sm text-text-soft">Folder: {current}</p>}
      <button type="button" onClick={() => void pick()} disabled={setFolder.isPending} className={PRIMARY}>
        {setFolder.isPending ? "Checking the folder…" : label}
      </button>
      <p className="m-0 text-xs text-muted">
        Use a folder your Drive / OneDrive / Dropbox app syncs; keep its files available offline.
      </p>
    </div>
  );
}

/** Browser sign-in; while waiting: "Finish signing in in your browser" + Cancel. */
export function SignInButton({ provider, label = "Sign in" }: { provider: CloudProviderId; label?: string }) {
  const signIn = useCloudSignIn();
  if (signIn.isPending) {
    return (
      <div role="status" className="flex flex-wrap items-center gap-3 text-sm text-text-soft">
        <span aria-hidden="true" className="size-4 animate-spin rounded-full border-2 border-accent border-t-transparent" />
        Finish signing in in your browser
        <button type="button" onClick={() => void cloudCommands.cancelSignIn()} className={CLOUD_BUTTON}>
          Cancel
        </button>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <button type="button" onClick={() => signIn.mutate(provider)} className={PRIMARY}>
        {label} with {CLOUD_PROVIDER_NAMES[provider]}
      </button>
      {signIn.isError && (
        <p role="alert" className="m-0 text-sm text-warning">
          {errorMessage(signIn.error)}
        </p>
      )}
    </div>
  );
}
