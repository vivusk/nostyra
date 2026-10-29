import { useEffect, useRef, useState } from "react";
import type { BindingKey } from "../../api/tauri-commands-gameplay";
import type { InputCatalog, InputDevice, LabelStyle } from "../../api/tauri-commands-input";
import { inputLabel, rejectReason } from "./controls-labels";
import { useGamepadCapture } from "./use-gamepad-capture";
import { useKeyboardCapture } from "./use-keyboard-capture";

const BUTTON = "h-11 cursor-pointer rounded-control px-4 font-body font-semibold";

interface ControlsCaptureDialogProps {
  device: InputDevice;
  target: BindingKey;
  /** Dialog title: the action's label as the Controls row shows it. */
  title: string;
  catalog: InputCatalog;
  labelStyle: LabelStyle;
  /** A bindable input was captured (the caller commits it via `assign_binding`). */
  onCapture: (input: string) => void;
  /** Cancel, Escape, or after a capture. */
  onClose: () => void;
}

/** Native modal "Press a key / button for <action>" dialog. Keyboard: keydown shows the
 *  key, keyup commits; Esc cancels. Gamepad: backend capture with a 5 s countdown. */
export function ControlsCaptureDialog({ device, target, title, catalog, labelStyle, onCapture, onClose }: ControlsCaptureDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [rejected, setRejected] = useState<string | null>(null);

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  /** Commits a valid input and closes; otherwise returns why it was refused. */
  const take = (input: string): string | null => {
    const reason = rejectReason(device, target, input, catalog);
    if (reason) return reason;
    onCapture(input);
    onClose();
    return null;
  };

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      aria-labelledby="controls-capture-title"
      className="m-auto w-[min(440px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-4 p-7">
        <h2 id="controls-capture-title" className="m-0 font-display text-[22px] font-semibold">
          {title}
        </h2>
        {device === "keyboard" ? (
          <KeyboardPrompt catalog={catalog} rejected={rejected} onCancel={onClose} onKey={(name) => setRejected(take(name))} />
        ) : (
          <GamepadPrompt labelStyle={labelStyle} onInput={take} />
        )}
        <div className="flex justify-end">
          <button type="button" onClick={onClose} className={`${BUTTON} border-0 bg-transparent text-text-soft hover:text-white`}>
            Cancel
          </button>
        </div>
      </div>
    </dialog>
  );
}

interface KeyboardPromptProps {
  catalog: InputCatalog;
  rejected: string | null;
  onCancel: () => void;
  onKey: (name: string) => void;
}

/** "Press a key" + the key being held + why the last key was refused. */
function KeyboardPrompt({ catalog, rejected, onCancel, onKey }: KeyboardPromptProps) {
  const { held, error } = useKeyboardCapture({ keyboardKeys: catalog.keyboardKeys, onCancel, onKey });
  const problem = error ?? rejected;
  return (
    <>
      <p className="m-0 leading-relaxed text-text-soft">
        {held ? (
          <>
            Release <b className="text-text">{inputLabel("keyboard", held, catalog, "xbox")}</b> to bind it.
          </>
        ) : (
          "Press a key. Esc cancels."
        )}
      </p>
      {problem && (
        <p role="alert" className="m-0 text-sm text-warning">
          {problem}
        </p>
      )}
      <p className="m-0 text-xs text-muted">Some keys (F10, Alt, the Windows key) never reach the launcher; set them in global.toml.</p>
    </>
  );
}

interface GamepadPromptProps {
  labelStyle: LabelStyle;
  onInput: (name: string) => string | null;
}

/** Countdown while the backend waits for a press; Try again after a timeout / refusal. */
function GamepadPrompt({ labelStyle, onInput }: GamepadPromptProps) {
  const { phase, secondsLeft, rejected, retry } = useGamepadCapture(onInput);
  const message =
    phase === "waiting" ? `Press a button on your gamepad… ${secondsLeft} s`
    : phase === "timed-out" ? "No button was pressed."
    : phase === "rejected" ? rejected
    : "The gamepad could not be read.";
  return (
    <>
      <p role={phase === "waiting" ? "status" : "alert"} className={`m-0 leading-relaxed ${phase === "waiting" ? "text-text-soft" : "text-warning"}`}>
        {message}
      </p>
      {phase !== "waiting" && (
        <button
          type="button"
          onClick={retry}
          autoFocus
          className={`${BUTTON} self-start border border-border-strong bg-transparent text-text-soft hover:text-white`}
        >
          Try again
        </button>
      )}
      <p className="m-0 text-xs text-muted">
        Buttons are stored by position; labels follow your last pad ({gamepadStyleName(labelStyle)}). Guide always opens the
        pause menu and cannot be bound.
      </p>
    </>
  );
}

const gamepadStyleName = (style: LabelStyle) => (style === "sony" ? "PlayStation" : "Xbox");
