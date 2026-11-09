// Features the web edition does not have (capability off in the UI): core
// download, RetroAchievements, logs / bug report, reveal in the file manager,
// memory card import, moving the data folder, the update checker, cloud saves.
// Queries answer "nothing"; actions reject with the desktop-only message.
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
  // Update checker: the web edition is always the deployed build (null = no status).
  get_update_status: constant(null),
  check_for_updates: desktopOnly,
  set_update_check_enabled: desktopOnly,
  skip_update_version: desktopOnly,
  open_update_release_page: desktopOnly,
  // Cloud saves: off on the web (the UI never calls these; capability `cloudSaves`).
  cloud_status: desktopOnly,
  cloud_set_local_folder: desktopOnly,
  cloud_sign_in: desktopOnly,
  cloud_cancel_sign_in: constant(undefined),
  cloud_sign_out: desktopOnly,
  cloud_game_status: constant({ state: "off", error: null }),
  cloud_sync_game: constant({ kind: "skipped", reason: "unsupported" }),
  cloud_resolve_conflict: desktopOnly,
  cloud_resolve_all: desktopOnly,
  cloud_sync_now: desktopOnly,
};
