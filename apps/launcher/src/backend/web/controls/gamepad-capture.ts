// Gamepad capture on the web (desktop: SDL3 thread, gamepad_capture_commands.rs):
// polls `navigator.getGamepads()` every animation frame for the first standard
// button pressed after the capture started (triggers count past half travel);
// buttons already held at the start are ignored until released, like the desktop.
// Pads without the "standard" layout cannot be named and are skipped.
import type { CapturedInput, LabelStyle } from "../../../api/tauri-commands-input";
import { labelStyleOfPadId, STANDARD_BUTTON_INPUTS } from "../../../lib/web-input-map";

/** Upper bound of one capture, as on the desktop. */
const MAX_CAPTURE_MS = 10_000;
const PRESSED_VALUE = 0.5;

let cancelRunning: (() => void) | null = null;
let lastStyle: LabelStyle = "xbox";

const isPressed = (button: GamepadButton) => button.pressed || button.value > PRESSED_VALUE;

/** Standard pads connected now. */
function standardPads(): Gamepad[] {
  return navigator.getGamepads().filter((pad): pad is Gamepad => pad !== null && pad.mapping === "standard");
}

/** Next pad press as a positional name, or null on timeout / cancel. */
export function captureGamepadInput(timeoutMs: number): Promise<CapturedInput | null> {
  cancelRunning?.();
  return new Promise((resolve) => {
    const held = new Set<string>();
    let frame = 0;
    const finish = (result: CapturedInput | null) => {
      cancelAnimationFrame(frame);
      clearTimeout(timer);
      cancelRunning = null;
      resolve(result);
    };
    const timer = setTimeout(() => finish(null), Math.min(timeoutMs, MAX_CAPTURE_MS));
    let first = true;
    const poll = () => {
      for (const pad of standardPads()) {
        for (let index = 0; index < STANDARD_BUTTON_INPUTS.length; index++) {
          const button = pad.buttons[index];
          const id = `${pad.index}:${index}`;
          if (!button || !isPressed(button)) {
            held.delete(id);
            continue;
          }
          if (first) held.add(id);
          if (held.has(id)) continue;
          lastStyle = labelStyleOfPadId(pad.id);
          return finish({ input: STANDARD_BUTTON_INPUTS[index], labelStyle: lastStyle });
        }
      }
      first = false;
      frame = requestAnimationFrame(poll);
    };
    cancelRunning = () => finish(null);
    poll();
  });
}

/** Ends a running capture (it resolves to null); no-op when idle. */
export function cancelGamepadCapture(): void {
  cancelRunning?.();
}

/** Labels of the last captured pad, else of the first connected one (default Xbox). */
export function gamepadLabelStyle(): LabelStyle {
  const pad = standardPads()[0];
  return lastStyle === "xbox" && pad ? labelStyleOfPadId(pad.id) : lastStyle;
}
