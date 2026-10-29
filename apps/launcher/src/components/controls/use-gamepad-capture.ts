// Gamepad capture for the Controls capture dialog: asks the backend (SDL, one capture
// at a time) for the next pad press with a 5 s countdown; cancelled on unmount.
import { useCallback, useEffect, useRef, useState } from "react";
import { GAMEPAD_CAPTURE_TIMEOUT_MS, inputCommands } from "../../api/tauri-commands-input";
import { useCaptureGamepadInput } from "../../hooks/use-input-catalog-query";

export type GamepadCapturePhase = "waiting" | "timed-out" | "rejected" | "failed";

/** Starts a capture on mount (and on `retry`). `onInput` gets the positional name and
 *  returns null when it took it, or the reason it cannot be bound (phase "rejected"). */
export function useGamepadCapture(onInput: (name: string) => string | null) {
  const capture = useCaptureGamepadInput();
  const [phase, setPhase] = useState<GamepadCapturePhase>("waiting");
  const [rejected, setRejected] = useState<string | null>(null);
  const [secondsLeft, setSecondsLeft] = useState(GAMEPAD_CAPTURE_TIMEOUT_MS / 1000);
  const running = useRef(false);
  const onInputRef = useRef(onInput);
  onInputRef.current = onInput;
  const { mutate } = capture;

  const start = useCallback(() => {
    running.current = true;
    setPhase("waiting");
    setSecondsLeft(GAMEPAD_CAPTURE_TIMEOUT_MS / 1000);
    mutate(GAMEPAD_CAPTURE_TIMEOUT_MS, {
      onSuccess: (captured) => {
        running.current = false;
        if (!captured) return setPhase("timed-out");
        const reason = onInputRef.current(captured.input);
        if (reason) {
          setRejected(reason);
          setPhase("rejected");
        }
      },
      // The hook's toast explains the error ("busy" while a game runs, SDL failure).
      onError: () => {
        running.current = false;
        setPhase("failed");
      },
    });
  }, [mutate]);

  // Deferred start: StrictMode's mount → unmount → mount must not start two captures
  // (the backend allows one at a time). Unmount cancels a running capture.
  useEffect(() => {
    const timer = window.setTimeout(start, 0);
    return () => {
      window.clearTimeout(timer);
      if (running.current) void inputCommands.cancelGamepadCapture();
      running.current = false;
    };
  }, [start]);

  // Countdown display while waiting.
  useEffect(() => {
    if (phase !== "waiting") return;
    const timer = window.setInterval(() => setSecondsLeft((s) => Math.max(0, s - 1)), 1000);
    return () => window.clearInterval(timer);
  }, [phase]);

  return { phase, secondsLeft, rejected, retry: start };
}
