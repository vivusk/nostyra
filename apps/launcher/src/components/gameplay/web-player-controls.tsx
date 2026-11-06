import { useMutation } from "@tanstack/react-query";
import { edition } from "../../backend";
import { errorMessage } from "../../api/tauri-commands";
import { webCommands } from "../../api/web-commands";
import { useGameSession } from "../../hooks/use-game-session";
import { useErrorToast } from "../error-toast";
import { WebSlotPickerOverlay } from "./web-slot-picker-overlay";

/** Web edition: what the launcher shows over the in-page player — an Exit button
 *  (writes the resume point and the last SRAM, then returns to the launcher) and
 *  the slot picker EmulatorJS's Save / Load buttons open. The desktop plays in the
 *  runner's own window and renders nothing here. */
export function WebPlayerControls() {
  const { data: session } = useGameSession();
  const { showError } = useErrorToast();
  const exit = useMutation({
    mutationFn: webCommands.exitGame,
    onError: (error) => showError("Cannot close the game", errorMessage(error)),
  });
  if (edition !== "web") return null;
  return (
    <>
      {session && (
        <button
          type="button"
          onClick={() => exit.mutate()}
          disabled={exit.isPending}
          className="fixed top-3 right-3 z-[55] h-10 cursor-pointer rounded-full border border-border-strong bg-surface-raised/90 px-5 font-body text-sm font-bold text-text shadow-xl hover:text-white disabled:cursor-default disabled:opacity-60"
        >
          {exit.isPending ? "Saving…" : "Exit game"}
        </button>
      )}
      <WebSlotPickerOverlay />
    </>
  );
}
