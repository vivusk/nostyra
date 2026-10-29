import { biosCommands, type CoreBios } from "../../api/tauri-commands-bios";
import { useAddBiosFile, useBiosStatus } from "../../hooks/use-bios-queries";

/** Settings → BIOS: per core that can use BIOS files, what each region boots (a
 *  real BIOS from the folder, or the built-in HLE BIOS), the files found, and
 *  "Add BIOS file…" / "Open folder". Nostyra never ships or downloads BIOS files. */
export function SettingsBiosSection() {
  const { data: cores = [] } = useBiosStatus();
  return (
    <div className="flex flex-col gap-5">
      {cores.map((core) => (
        <CoreBiosBlock key={core.coreId} core={core} />
      ))}
      <p className="m-0 text-xs text-muted">
        Nostyra does not include or download BIOS files. Use a BIOS dumped from your own console.
      </p>
    </div>
  );
}

function CoreBiosBlock({ core }: { core: CoreBios }) {
  const addBios = useAddBiosFile();
  const usable = core.files.filter((f) => f.valid);
  return (
    <div className="flex flex-col gap-3">
      <h3 className="m-0 text-sm font-bold">
        {core.systemNames.join(", ")}
        <span className="ml-2 font-medium text-muted">{core.coreName}</span>
      </h3>
      <ul className="m-0 flex list-none flex-col gap-1 p-0 text-sm text-text-soft">
        {core.regions.map((r) => (
          <li key={r.region}>
            <b className="text-text">{r.label}</b>:{" "}
            {r.fileName ? `Real BIOS: ${r.fileName}${modelOf(core, r.fileName)}` : "Built-in HLE BIOS — some games need a real BIOS"}
          </li>
        ))}
      </ul>
      {core.files.some((f) => !f.valid) && (
        <p className="m-0 text-xs text-warning">
          Ignored (not a valid or known BIOS): {core.files.filter((f) => !f.valid).map((f) => f.fileName).join(", ")}
        </p>
      )}
      {usable.length === 0 && (
        <p className="m-0 text-xs text-muted">No BIOS file yet. Games start with the built-in HLE BIOS.</p>
      )}
      <p className="m-0 text-xs break-all text-muted">Folder: {core.folder}</p>
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          onClick={() => addBios.mutate(core.coreId)}
          disabled={addBios.isPending}
          className="h-11 cursor-pointer rounded-control border-0 bg-accent-strong px-5 font-body text-sm font-bold text-white hover:bg-accent disabled:cursor-default disabled:opacity-50"
        >
          Add BIOS file…
        </button>
        <button
          type="button"
          onClick={() => void biosCommands.openSystemFolder()}
          className="h-11 cursor-pointer rounded-control border border-border-strong bg-transparent px-5 font-body text-sm font-bold text-text-soft hover:text-white"
        >
          Open folder
        </button>
      </div>
    </div>
  );
}

/** " (SCPH-1001, US)" for a file the backend identified, else "". */
function modelOf(core: CoreBios, fileName: string) {
  const file = core.files.find((f) => f.fileName === fileName);
  if (!file?.model) return "";
  return ` (${file.model}, ${file.region})`;
}
