import type { CloudStatus } from "../../api/tauri-commands-cloud";
import { useCloudStatus } from "../../hooks/use-cloud-queries";
import { SettingsCloudAccountRow } from "./settings-cloud-account-row";
import { LocalFolderPicker, SettingsCloudProviderPicker, SignInButton } from "./settings-cloud-provider-picker";

/** Settings → Cloud saves (desktop; gated by `capabilities.cloudSaves`): the "Sign in
 *  again" banner, the provider picker, the signed-in account row, the privacy line. */
export function SettingsCloudSavesSection() {
  const { data: status, isError } = useCloudStatus();

  if (!status) {
    return <p className="m-0 text-sm text-text-soft">{isError ? "The cloud saves status could not be read." : "…"}</p>;
  }
  return (
    <>
      {status.needsSignIn && <SignInAgainBanner status={status} />}
      {/* Remount on a provider change (sign-out resets the select to the saved value). */}
      <SettingsCloudProviderPicker key={status.provider} status={status} />
      {status.signedIn && <SettingsCloudAccountRow status={status} />}
      <p className="m-0 text-xs text-muted">
        Saves go to your own account or folder; Nostyra has no server. Games never wait more than a few
        seconds for the cloud.
      </p>
    </>
  );
}

/** The stored sign-in stopped working (expired token, unreachable folder, provider
 *  not built into this version): saves stay untouched until the user acts. */
function SignInAgainBanner({ status }: { status: CloudStatus }) {
  const reason = status.signInReason ?? "Sign in again.";
  return (
    <div role="alert" className="flex flex-col gap-2 rounded-control border border-warning/40 bg-surface-raised px-4 py-3 text-sm">
      <p className="m-0 font-medium text-text">Cloud saves paused — {reason}</p>
      {status.provider === "local_folder" ? (
        <LocalFolderPicker current={status.localFolder} label="Choose the folder again" />
      ) : (
        status.providerAvailable && <SignInButton provider={status.provider} label="Sign in again" />
      )}
    </div>
  );
}
