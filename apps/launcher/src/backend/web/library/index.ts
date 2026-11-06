// Library, folders, systems, scan, detail/home, metadata and artwork commands of the
// web backend, composed from their modules.
import { artworkCommands } from "../artwork/artwork-commands";
import { mergeTables, type CommandTable } from "../web-command-table";
import { detailCommands } from "./detail-commands";
import { homeCommands } from "./home-commands";
import { libraryCommands as listAndScanCommands } from "./library-commands";

export const libraryCommands: CommandTable = mergeTables(listAndScanCommands, detailCommands, homeCommands, artworkCommands);
