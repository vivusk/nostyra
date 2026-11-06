// Gameplay settings (config layers) and the Controls editor (catalog, bindings,
// gamepad capture) on the web: the shared TOML files of the Data folder.
import { gameplayConfigCommands } from "../config/gameplay-config-commands";
import { controlsCommands } from "../controls/controls-commands";
import { mergeTables, type CommandTable } from "../web-command-table";

export const gameplayCommands: CommandTable = mergeTables(gameplayConfigCommands, controlsCommands);
