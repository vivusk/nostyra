import type { ReactNode } from "react";
import { AddFolderButton } from "../components/add-folder-button";
import { LastScanProblemsLink } from "../components/scan-problems-dialog";
import { SettingsBiosSection } from "../components/settings/settings-bios-section";
import { SettingsCoresSection } from "../components/settings/settings-cores-section";
import { SettingsDataFolderSection } from "../components/settings/settings-data-folder-section";
import { SettingsLogsRow } from "../components/settings/settings-logs-row";
import { SettingsArtworkSection } from "../components/settings-artwork-section";
import { ControlsSection } from "../components/settings/controls-section";
import { GameplayDefaultsSection } from "../components/settings/gameplay-defaults-section";
import { SettingsAchievementsSection } from "../components/settings/settings-achievements-section";
import { useLibraryFolders, useRemoveLibraryFolder, useScanProgress, useStartScan } from "../hooks/use-library-queries";
import { PageHeader } from "./page-header";

/** Settings: folders, core, BIOS, gameplay defaults, RetroAchievements, controls, artwork
 *  cache, data folder, attribution, logs / bug report. */
export function SettingsPage() {
  const { data: folders = [] } = useLibraryFolders();
  const { data: scan } = useScanProgress();
  const removeFolder = useRemoveLibraryFolder();
  const startScan = useStartScan();

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <PageHeader title="Settings" />

      <SettingsSection title="ROM folders">
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {folders.map((folder) => (
            <li key={folder.id} className="flex items-center gap-3 rounded-control bg-surface-raised px-4 py-2 text-sm">
              <span className="flex-1 break-all">{folder.path}</span>
              <button
                type="button"
                onClick={() => removeFolder.mutate(folder.id)}
                aria-label={`Remove ${folder.path}`}
                className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white"
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
        <p className="m-0 text-xs text-muted">Removing a folder hides its games; play history comes back if you add it again.</p>
        <LastScanProblemsLink />
        <div className="flex flex-wrap gap-3">
          <AddFolderButton />
          <button
            type="button"
            onClick={() => startScan.mutate()}
            disabled={Boolean(scan)}
            className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
          >
            {scan ? "Scanning…" : "Rescan library"}
          </button>
        </div>
      </SettingsSection>

      <SettingsSection title="Emulator cores">
        <SettingsCoresSection />
      </SettingsSection>

      <SettingsSection title="BIOS">
        <SettingsBiosSection />
      </SettingsSection>

      <SettingsSection title="Gameplay defaults">
        <GameplayDefaultsSection />
      </SettingsSection>

      <SettingsSection title="RetroAchievements">
        <SettingsAchievementsSection />
      </SettingsSection>

      <SettingsSection title="Controls">
        <ControlsSection />
      </SettingsSection>

      <SettingsSection title="Artwork">
        <SettingsArtworkSection />
      </SettingsSection>

      <SettingsSection title="Data folder">
        <SettingsDataFolderSection />
      </SettingsSection>

      <SettingsSection title="About">
        <p className="m-0 text-sm text-text-soft">
          Game data: libretro-database / No-Intro, CC-BY-SA-4.0. Metadata: libretro-database
          (CC-BY-SA-4.0). Artwork: libretro-thumbnails, downloaded per game into a local cache.
          Nostyra does not ship or download games.
        </p>
        <SettingsLogsRow />
      </SettingsSection>
    </div>
  );
}

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3 rounded-card border border-border bg-surface p-6">
      <h2 className="m-0 font-display text-lg font-semibold">{title}</h2>
      {children}
    </section>
  );
}
