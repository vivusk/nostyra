import { useState } from "react";
import type { GameplayConfig, GameplayConfigKey } from "../../api/tauri-commands-gameplay";
import { netplayCommands } from "../../api/tauri-commands-netplay";
import { useGameplayConfig, useSetGameplayConfigValue } from "../../hooks/use-gameplay-config-queries";
import { SelectControl, SettingRow } from "../gameplay/gameplay-setting-controls";

const DELAYS = [
  { value: "auto", label: "Auto (from ping)" },
  ...Array.from({ length: 11 }, (_, n) => ({ value: String(n), label: n === 1 ? "1 frame" : `${n} frames` })),
];

const FIELD =
  "h-10 w-72 max-w-full rounded-control border border-border-strong bg-surface-raised px-3 font-body text-sm text-text outline-none focus:border-accent";

/** Settings → Netplay (desktop): nickname, input delay, HUD, "Prefer relay", and the
 *  server address under Advanced. Global `[netplay]` keys; apply to the next room. */
export function SettingsNetplaySection() {
  const { data: view } = useGameplayConfig(null);
  const setValue = useSetGameplayConfigValue();
  if (!view) return null;
  const config = view.resolved;
  const save = <K extends Extract<GameplayConfigKey, `netplay.${string}`>>(key: K, value: GameplayConfig[K] | null) =>
    setValue.mutate({ scope: "global", key, value });

  return (
    <>
      <SettingRow controlId="netplay-nickname" label="Nickname" help="Shown to the others in the room. Empty: your Windows user name.">
        <TextSetting key={config["netplay.nickname"]} id="netplay-nickname" value={config["netplay.nickname"]} maxLength={32} onSave={(v) => save("netplay.nickname", v)} />
      </SettingRow>
      <SettingRow controlId="netplay-delay" label="Input delay" help="Auto picks 1–4 frames from the ping; more delay means fewer rollbacks.">
        <SelectControl
          controlId="netplay-delay"
          choices={DELAYS}
          value={config["netplay.input_delay"]}
          onChange={(v) => save("netplay.input_delay", v ?? "auto")}
        />
      </SettingRow>
      <Toggle
        label="Show the online HUD in game"
        help="Ping, rollbacks and relay; warnings show either way."
        checked={config["netplay.hud"]}
        onChange={(v) => save("netplay.hud", v)}
      />
      <Toggle
        label="Prefer relay (best effort)"
        help="Connects through the relay server when it can; may still connect directly, so the others may see your IP address."
        checked={config["netplay.relay_only"]}
        onChange={(v) => save("netplay.relay_only", v)}
      />
      <details className="text-sm text-text-soft">
        <summary className="cursor-pointer font-semibold">Advanced</summary>
        <div className="mt-3">
          <SettingRow controlId="netplay-server" label="Server address" help="Clear it to use the Nostyra server again.">
            <TextSetting key={config["netplay.server_url"]} id="netplay-server" value={config["netplay.server_url"]} maxLength={200} onSave={(v) => save("netplay.server_url", v || null)} />
          </SettingRow>
        </div>
      </details>
      <p className="m-0 text-xs text-muted">
        Everyone in a room (players and spectators) may learn each other's IP address.{" "}
        <button type="button" onClick={() => void netplayCommands.openPrivacy()} className="cursor-pointer border-0 bg-transparent p-0 font-body text-xs text-link underline">
          Privacy
        </button>
      </p>
    </>
  );
}

/** Text field saved on blur / Enter (only when changed). */
function TextSetting({ id, value, maxLength, onSave }: { id: string; value: string; maxLength: number; onSave: (value: string) => void }) {
  const [draft, setDraft] = useState(value);
  const commit = () => {
    const next = draft.trim();
    if (next !== value) onSave(next);
  };
  return (
    <input
      id={id}
      value={draft}
      maxLength={maxLength}
      spellCheck={false}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
      className={FIELD}
    />
  );
}

function Toggle({ label, help, checked, onChange }: { label: string; help: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="flex cursor-pointer items-start gap-3 text-sm text-text-soft">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="mt-0.5 h-4 w-4 accent-[var(--color-accent)]" />
      <span className="flex flex-col">
        {label}
        <span className="text-xs text-muted">{help}</span>
      </span>
    </label>
  );
}
