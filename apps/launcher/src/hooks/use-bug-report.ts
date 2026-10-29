// Logs folder and bug-report zip (Settings → About, game error toasts).
import { useMutation } from "@tanstack/react-query";
import { errorMessage } from "../api/tauri-commands";
import { logCommands } from "../api/tauri-commands-logs";
import { useErrorToast } from "../components/error-toast";

/** Toast shown once the zip is written and revealed in the file manager. */
export const BUG_REPORT_SAVED = "Bug report saved — attach it to your GitHub issue";

/** Creates the bug-report zip; the backend reveals it, the toast says what to do next. */
export function useCreateBugReport() {
  const { showError, showNotice } = useErrorToast();
  return useMutation({
    mutationFn: logCommands.createBugReport,
    onSuccess: () => showNotice(BUG_REPORT_SAVED),
    onError: (error) => showError("Cannot create the bug report", errorMessage(error)),
  });
}

export function useOpenLogsFolder() {
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: logCommands.openLogsFolder,
    onError: (error) => showError("Cannot open the folder", errorMessage(error)),
  });
}
