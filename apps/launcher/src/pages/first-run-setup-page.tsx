import type { ReactNode } from "react";
import { edition } from "../backend";
import { AddFolderButton } from "../components/add-folder-button";
import { ChangeRomFolderButton } from "../components/change-rom-folder-button";
import { NostyraLogoMark } from "../components/sidebar-icons";
import { WebDataLocationStep } from "../components/web-data-location-step";
import { WebPrivacyNote } from "../components/web-licences-link";
import { useLibraryFolders, useSetupState } from "../hooks/use-library-queries";
import { supportedSystemsText, useSystems } from "../hooks/use-system-queries";

/** Pick a ROM folder (scan starts at once); on the web, then where the Data folder
 *  lives. The shell then switches to the Library. No core is needed up front: each
 *  is offered on the first Play of its system. */
export function FirstRunSetupPage() {
  const { data: setup } = useSetupState();
  const { data: folders } = useLibraryFolders();
  const { data: systems } = useSystems();
  return (
    <main className="flex h-full items-center justify-center overflow-y-auto p-10">
      <div className="flex w-full max-w-xl flex-col gap-8">
        <header className="flex flex-col gap-3">
          <div className="flex items-center gap-2.5">
            <NostyraLogoMark />
            <span className="font-display text-xl font-bold tracking-[0.14em]">NOSTYRA</span>
          </div>
          <h1 className="m-0 font-display text-4xl font-bold tracking-[-0.02em]">Set up your library</h1>
          <p className="m-0 text-muted">
            Old Worlds. New Ways. {edition === "web" ? "Two quick steps" : "One quick step"} and you are playing.
          </p>
        </header>

        <SetupStep number={1} title="Choose your ROM folder" done={Boolean(setup?.hasFolders)}>
          {setup?.hasFolders ? (
            <div className="flex flex-wrap items-center gap-3">
              <p className="m-0 min-w-0 flex-1 text-sm break-all text-text-soft">
                {edition === "web" ? "Folder: " : ""}
                <b className="text-text">{folders?.map((f) => f.path).join(", ")}</b>
                {edition === "web" && (
                  <span className="block text-xs text-muted">The browser shows only the folder's name, not its full path.</span>
                )}
              </p>
              {folders && <ChangeRomFolderButton current={folders} />}
            </div>
          ) : (
            <>
              <p className="m-0 text-sm text-text-soft">
                Nostyra finds {supportedSystemsText(systems)} games in it and every subfolder, however
                you organise them. Use ROMs you own.
              </p>
              <AddFolderButton label="Choose folder" />
            </>
          )}
        </SetupStep>
        {edition === "web" && setup?.hasFolders && (
          <SetupStep number={2} title="Where to keep saves and settings" done={false}>
            <WebDataLocationStep />
          </SetupStep>
        )}
        <WebPrivacyNote />
      </div>
    </main>
  );
}

function SetupStep({ number, title, done, children }: { number: number; title: string; done: boolean; children: ReactNode }) {
  return (
    <section className="flex gap-4 rounded-card border border-border bg-surface p-6">
      <span
        aria-hidden="true"
        className={`flex size-8 shrink-0 items-center justify-center rounded-full font-display text-sm font-bold ${done ? "bg-accent text-app-bg" : "bg-border-strong text-text"}`}
      >
        {done ? "✓" : number}
      </span>
      <div className="flex flex-1 flex-col gap-3">
        <h2 className="m-0 font-display text-lg font-semibold">
          {title}
          {done && <span className="sr-only"> (done)</span>}
        </h2>
        {children}
      </div>
    </section>
  );
}
