// DOM `KeyboardEvent.code` → the winit key name the config stores. They are equal
// for every supported key except the OS key (DOM `Meta*`, winit `Super*`). The
// backend catalog (`keyboardKeys`) stays the authority: a name it does not list
// cannot be bound (and `assign_binding` rejects it anyway).

const DOM_TO_WINIT: Record<string, string> = {
  MetaLeft: "SuperLeft",
  MetaRight: "SuperRight",
};

/** The config key name for a DOM `code`, or null when the catalog does not offer it
 *  (unknown / `Unidentified` keys). */
export function keyNameFromCode(code: string, keyboardKeys: readonly string[]): string | null {
  const name = DOM_TO_WINIT[code] ?? code;
  return keyboardKeys.includes(name) ? name : null;
}

/** Whether a keydown should be ignored by a capture (auto-repeat, IME composition). */
export function isIgnorableKeyEvent(event: Pick<KeyboardEvent, "repeat" | "isComposing" | "code">): boolean {
  return event.repeat || event.isComposing || event.code === "" || event.code === "Unidentified";
}
