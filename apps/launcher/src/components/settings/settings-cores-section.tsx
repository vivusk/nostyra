import { useSetupState } from "../../hooks/use-library-queries";
import { systemLabel, useSystems } from "../../hooks/use-system-queries";
import { CoreDownloadPanel } from "../core-download-panel";

/** Settings → Cores: every emulator core, the systems it plays, status and
 *  install / re-download. A missing core is also offered on the first Play of its system. */
export function SettingsCoresSection() {
  const { data: setup } = useSetupState();
  const { data: systems } = useSystems();
  return (
    <div className="flex flex-col gap-5">
      {setup?.cores.map((core) => (
        <div key={core.id} className="flex flex-col gap-2">
          <h3 className="m-0 text-sm font-bold">
            {core.displayName}
            <span className="ml-2 font-medium text-muted">
              {core.systems.map((id) => systemLabel(systems, id)).join(", ")}
            </span>
          </h3>
          <CoreDownloadPanel core={core} />
        </div>
      ))}
    </div>
  );
}
