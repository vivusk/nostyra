// Features the web edition does not have (capability off in the UI): core
// download, RetroAchievements, logs / bug report, reveal in the file manager,
// memory card import, moving the data folder. Queries answer "nothing"; actions
// reject with the desktop-only message.
import { constant, desktopOnly, type CommandTable } from "../web-command-table";

export const desktopOnlyCommands: CommandTable = {
  get_core_status: constant({ installed: true, path: null, version: null, downloading: false }),
  download_core: desktopOnly,
  achievements_capabilities: constant({ hardcoreUi: false }),
  game_achievements: desktopOnly,
  ra_account: constant(null),
  ra_login: desktopOnly,
  ra_logout: constant(undefined),
  open_logs_folder: desktopOnly,
  create_bug_report: desktopOnly,
  reveal_rom_in_folder: desktopOnly,
  open_system_folder: desktopOnly,
  get_memory_cards: constant([]),
  import_memory_card: desktopOnly,
  check_data_folder: desktopOnly,
  change_data_folder: desktopOnly,
};
