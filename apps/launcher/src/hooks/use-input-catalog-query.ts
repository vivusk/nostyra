// Controls editor reads: the binding catalog (static), the pad label style and
// gamepad capture. Writes (assign / set / reset) live in use-gameplay-config-queries.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { inputCommands, type CapturedInput, type LabelStyle } from "../api/tauri-commands-input";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Action / hotkey / input names, pad labels, reserved inputs and defaults. */
export function useInputCatalog() {
  return useQuery({
    queryKey: queryKeys.inputCatalog,
    queryFn: inputCommands.getInputCatalog,
    staleTime: Infinity,
  });
}

/** "xbox" | "sony": labels of the last captured pad (Xbox until a capture). */
export function useGamepadLabelStyle() {
  return useQuery({
    queryKey: queryKeys.gamepadLabelStyle,
    queryFn: inputCommands.gamepadLabelStyle,
    staleTime: Infinity,
  });
}

/** Waits for one pad press (`null` = timed out / cancelled); the captured pad's label
 *  style updates the cached style. Cancel with `inputCommands.cancelGamepadCapture()`. */
export function useCaptureGamepadInput() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: (timeoutMs?: number) => inputCommands.captureGamepadInput(timeoutMs),
    onSuccess: (captured: CapturedInput | null) => {
      if (captured) queryClient.setQueryData<LabelStyle>(queryKeys.gamepadLabelStyle, captured.labelStyle);
    },
    onError: (error) => showError("Cannot read the gamepad", errorMessage(error)),
  });
}
