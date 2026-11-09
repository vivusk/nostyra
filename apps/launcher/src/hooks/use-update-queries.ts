// Update checker: one status query per launch (the automatic check runs inside it),
// user actions write their returned status into the same cache. `updateStatus` is
// never invalidated: a refetch would re-run the network check.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capabilities } from "../backend";
import { errorMessage } from "../api/tauri-commands";
import { updateCommands, type UpdateStatus } from "../api/tauri-commands-updates";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Status (null = none, e.g. the web edition). Errors stay silent (no toast). */
export function useUpdateStatus() {
  return useQuery<UpdateStatus | null>({
    queryKey: queryKeys.updateStatus,
    queryFn: updateCommands.getUpdateStatus,
    enabled: capabilities.updateCheck,
    staleTime: Infinity,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

/** Writes a command's returned status into the query cache. Only "Check now" decides
 *  `checkFailed`; toggle/skip answers keep the previous check's failure line. */
function useStoreStatus(fromCheck = false) {
  const queryClient = useQueryClient();
  return (status: UpdateStatus) =>
    queryClient.setQueryData<UpdateStatus | null>(queryKeys.updateStatus, (prev) =>
      fromCheck ? status : { ...status, checkFailed: status.checkFailed || (prev?.checkFailed ?? false) },
    );
}

/** "Check now"; a GitHub failure comes back as `checkFailed`, not as an error. */
export function useCheckForUpdates() {
  const store = useStoreStatus(true);
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: updateCommands.checkForUpdates,
    onSuccess: store,
    onError: (error) => showError("Cannot check for updates", errorMessage(error)),
  });
}

export function useSetUpdateCheckEnabled() {
  const store = useStoreStatus();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: updateCommands.setUpdateCheckEnabled,
    onSuccess: store,
    onError: (error) => showError("Cannot save the setting", errorMessage(error)),
  });
}

export function useSkipUpdateVersion() {
  const store = useStoreStatus();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: updateCommands.skipUpdateVersion,
    onSuccess: store,
    onError: (error) => showError("Cannot skip this version", errorMessage(error)),
  });
}

export function useOpenUpdateReleasePage() {
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: updateCommands.openUpdateReleasePage,
    onError: (error) => showError("Cannot open the release page", errorMessage(error)),
  });
}
