// Settings → BIOS on the web (src-tauri bios_commands.rs): per browser core that can
// use BIOS files, the files in the Data folder's `system/` and what each region
// boots; "Add BIOS file…" copies a picked file there (never over a different one).
import type { CoreBios } from "../../../api/tauri-commands-bios";
import { webSystems } from "../catalog/web-systems";
import { currentDataDir, dataFolderStatus } from "../fs/data-folder";
import { takePickedFile } from "../fs/picked-files";
import { biosFolder, biosImportName, coreBiosView } from "../player/bios-files";
import { commandError, type CommandTable } from "../web-command-table";

/** Browser systems whose core uses BIOS files, one per core. */
async function biosSystems() {
  const systems = (await webSystems()).filter((system) => system.firmware.length > 0);
  return systems.filter((system, index) => systems.findIndex((other) => other.coreId === system.coreId) === index);
}

async function biosStatus(): Promise<CoreBios[]> {
  const data = await currentDataDir();
  const status = await dataFolderStatus();
  const label = status.name ?? "Browser storage";
  const all = await webSystems();
  return Promise.all(
    (await biosSystems()).map((system) => {
      const names = all.filter((other) => other.coreId === system.coreId).map((other) => other.displayName);
      return coreBiosView(data, system, names, label);
    }),
  );
}

async function addBiosFile(coreId: string, ref: string): Promise<string> {
  const system = (await biosSystems()).find((candidate) => candidate.coreId === coreId);
  if (!system) throw commandError("invalid", "This core uses no BIOS files.");
  const file = takePickedFile(ref);
  if (!file) throw commandError("invalid", "Choose the file again.");
  const maxSize = Math.max(...system.firmware.map((f) => f.size));
  const image = new Uint8Array(await file.slice(0, maxSize + 1).arrayBuffer());
  let name: string;
  try {
    name = biosImportName(file.name, image, system.firmware, system.displayName);
  } catch (error) {
    throw commandError("invalid", (error as Error).message);
  }
  const data = await currentDataDir();
  const path = `${biosFolder(system.firmware)}/${name}`;
  const existing = await data.readFile(path);
  if (existing) {
    const same = existing.size === image.length && new Uint8Array(await existing.arrayBuffer()).every((b, i) => b === image[i]);
    if (same) return name;
    throw commandError(
      "invalid",
      `A different file named ${name} is already in the BIOS folder. Remove or rename it first.`,
    );
  }
  await data.writeFile(path, image).catch((error) => {
    throw commandError("storage", `Cannot write to the BIOS folder: ${String(error)}`);
  });
  return name;
}

export const biosCommands: CommandTable = {
  get_bios_status: () => biosStatus(),
  add_bios_file: ({ coreId, path }) => addBiosFile(String(coreId), String(path)),
};
