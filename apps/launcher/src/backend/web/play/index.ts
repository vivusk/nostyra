// Play via EmulatorJS, the active session, save states, resume point and BIOS on the
// web (desktop play_commands.rs, save_state_commands.rs, resume_commands.rs,
// bios_commands.rs), plus the web-only player commands: Exit, the slot picker's
// answer and the parked-SRAM banner (`api/web-commands.ts`).
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";
import { activeGameId, exitSession, startSession } from "../player/active-session";
import { pendingSavePaths, retryPendingSaves } from "../player/pending-saves";
import { chooseSlot } from "../player/slot-picker";
import { mergeTables, type CommandTable } from "../web-command-table";
import { biosCommands } from "./bios-commands";
import { stateCommands } from "./state-commands";

const sessionCommands: CommandTable = {
  play_game: ({ gameId, loadState }) => startSession(Number(gameId), (loadState ?? null) as StateSlotId | null),
  get_active_session: async () => activeGameId(),
  web_exit_game: async () => exitSession(),
  web_slot_picker_choose: async ({ slot }) => chooseSlot((slot ?? null) as StateSlotId | null),
  web_pending_saves: () => pendingSavePaths(),
  web_retry_pending_saves: () => retryPendingSaves(activeGameId() !== null),
};

export const playCommands: CommandTable = mergeTables(sessionCommands, stateCommands, biosCommands);
