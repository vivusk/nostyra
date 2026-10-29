// Display name of a save slot: "Slot 3", or "Resume point" for the auto slot.
import type { StateSlotId } from "../api/tauri-commands-gameplay";

export function saveSlotLabel(slot: StateSlotId): string {
  return slot === "auto" ? "Resume point" : `Slot ${slot}`;
}
