import type { RaAccount } from "../../api/tauri-commands-achievements";
import type { GameplayConfigKey } from "../../api/tauri-commands-gameplay";
import { useAchievementsCapabilities, useRaAccount, useRaLogout } from "../../hooks/use-achievements-queries";
import { useGameplayConfig, useSetGameplayConfigValue } from "../../hooks/use-gameplay-config-queries";
import { SettingsAchievementsLoginForm } from "./settings-achievements-login-form";

type ToggleKey = Extract<GameplayConfigKey, `achievements.${string}`>;

interface Toggle {
  key: ToggleKey;
  label: string;
  help?: string;
}

const TOGGLES: Toggle[] = [
  { key: "achievements.enabled", label: "Enable achievements", help: "Off: games start without RetroAchievements" },
  { key: "achievements.sound", label: "Unlock sound" },
  { key: "achievements.leaderboard_popups", label: "Leaderboard pop-ups", help: "Start, fail and submit messages in game" },
  {
    key: "achievements.patched_rom_fallback",
    label: "Use original ROM achievements for patched games (softcore)",
    help: "Achievements may misfire if the patch changes game memory.",
  },
];

const HARDCORE: Toggle = {
  key: "achievements.hardcore",
  label: "Hardcore",
  help: "Hardcore: no save-state loading, rewind or resume; requires restart of the game",
};

/** Settings → RetroAchievements: login (or account + Log out), the unencrypted-token
 *  notice, and the global `[achievements]` toggles. Hardcore appears only when the
 *  build allows it (`achievements_capabilities`). */
export function SettingsAchievementsSection() {
  const { data: account, isPending } = useRaAccount();
  const { data: capabilities } = useAchievementsCapabilities();
  const { data: view } = useGameplayConfig(null);
  const setValue = useSetGameplayConfigValue();
  const toggles = capabilities?.hardcoreUi ? [...TOGGLES, HARDCORE] : TOGGLES;

  return (
    <>
      {isPending ? null : account ? <AccountLine account={account} /> : <SettingsAchievementsLoginForm />}
      <p className="m-0 text-xs text-muted">
        Your password is used once and never stored. Your login token is saved unencrypted in
        &lt;data folder&gt;/achievements/credentials.json — anyone with access to this PC account can
        read it; Log out deletes it.
      </p>
      {view && (
        <div className="flex flex-col gap-2">
          {toggles.map(({ key, label, help }) => (
            <label key={key} className="flex cursor-pointer items-start gap-3 text-sm text-text-soft">
              <input
                type="checkbox"
                checked={Boolean(view.resolved[key])}
                onChange={(e) => setValue.mutate({ scope: "global", key, value: e.target.checked })}
                className="mt-0.5 h-4 w-4 accent-[var(--color-accent)]"
              />
              <span className="flex flex-col">
                {label}
                {help && <span className="text-xs text-muted">{help}</span>}
              </span>
            </label>
          ))}
        </div>
      )}
      <p className="m-0 text-xs text-muted">Changes apply next time you start a game.</p>
    </>
  );
}

function AccountLine({ account }: { account: RaAccount }) {
  const logout = useRaLogout();
  // Known only right after a login in this launcher run (the file keeps no points).
  const points =
    account.points !== null ? ` (${account.points} points · ${account.softcorePoints ?? 0} softcore)` : "";
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-control bg-surface-raised px-4 py-2 text-sm">
      <span className="flex-1">
        Logged in as <strong>{account.user}</strong>
        {points}
      </span>
      <button
        type="button"
        onClick={() => logout.mutate()}
        disabled={logout.isPending}
        className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
      >
        Log out
      </button>
    </div>
  );
}
