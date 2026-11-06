// EmulatorJS's in-game Save / Load buttons open Nostyra's slot picker, a React
// overlay in the launcher page (components/gameplay/web-slot-picker-overlay.tsx):
// the backend asks with the `web-slot-picker` event and the overlay answers with the
// `web_slot_picker_choose` command (`null` = cancelled).
import { WEB_EVENTS, type WebSlotPickerRequest } from "../../../api/web-commands";
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";
import { emit } from "../web-event-bus";


let answer: ((slot: StateSlotId | null) => void) | null = null;

/** Shows the picker; resolves to the chosen slot, or null when cancelled / closed. */
export function pickSlot(gameId: number, mode: "save" | "load"): Promise<StateSlotId | null> {
  answer?.(null);
  emit<WebSlotPickerRequest>(WEB_EVENTS.slotPicker, { gameId, mode });
  return new Promise((resolve) => {
    answer = resolve;
  });
}

/** The overlay's answer. */
export function chooseSlot(slot: StateSlotId | null): void {
  const resolve = answer;
  answer = null;
  resolve?.(slot);
}

/** Closes an open picker (the session ended). */
export function closeSlotPicker(): void {
  if (!answer) return;
  chooseSlot(null);
  emit<WebSlotPickerRequest>(WEB_EVENTS.slotPicker, null);
}
