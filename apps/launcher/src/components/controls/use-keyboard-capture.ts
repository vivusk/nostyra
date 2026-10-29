// Keyboard capture for the Controls capture dialog: the key is taken on keydown and
// committed on its keyup (so the release never reaches the page as a new action).
// Listens on window in the capture phase and stops the event, so app shortcuts
// (Alt+Left back, Escape on Detail, dialog Escape) never see captured keys.
import { useEffect, useRef, useState } from "react";
import { isIgnorableKeyEvent, keyNameFromCode } from "../../lib/keyboard-event-code";

interface KeyboardCaptureOptions {
  /** Catalog `keyboardKeys`: anything else cannot be bound. */
  keyboardKeys: readonly string[];
  /** Escape pressed: cancel the capture. */
  onCancel: () => void;
  /** A bindable key went down and up again (winit name). */
  onKey: (name: string) => void;
}

/** Returns the held key name (shown while held) and an inline error for unknown keys. */
export function useKeyboardCapture({ keyboardKeys, onCancel, onKey }: KeyboardCaptureOptions) {
  const [held, setHeld] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The held key lives in a ref too, so a fast tap's keyup sees it before a re-render.
  const heldRef = useRef<{ code: string; name: string } | null>(null);
  // Latest callbacks for the listeners registered once.
  const latest = useRef({ keyboardKeys, onCancel, onKey });
  latest.current = { keyboardKeys, onCancel, onKey };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.code === "Escape") return latest.current.onCancel();
      if (isIgnorableKeyEvent(event)) return;
      const name = keyNameFromCode(event.code, latest.current.keyboardKeys);
      if (!name) {
        heldRef.current = null;
        setHeld(null);
        setError(`This key (${event.code || event.key}) cannot be bound here.`);
        return;
      }
      setError(null);
      heldRef.current = { code: event.code, name };
      setHeld(name);
    };
    const onKeyUp = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const current = heldRef.current;
      if (current && current.code === event.code) {
        heldRef.current = null;
        setHeld(null);
        latest.current.onKey(current.name);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
  }, []);

  return { held, error };
}
