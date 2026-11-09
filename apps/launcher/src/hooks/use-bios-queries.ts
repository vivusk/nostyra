// BIOS status (Settings) and memory cards (Detail → Save states).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "../backend";
import { biosCommands, type CoreBios } from "../api/tauri-commands-bios";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

export function useBiosStatus() {
  return useQuery({ queryKey: queryKeys.bios, queryFn: biosCommands.getBiosStatus });
}

/** Picks a file and copies it into the BIOS folder (the backend validates it). The
 *  picker is filtered to the core's extensions when it names some (PS2: .bin / .rom0),
 *  with an "All files" escape. */
export function useAddBiosFile() {
  const queryClient = useQueryClient();
  const { showError, showNotice } = useErrorToast();
  return useMutation({
    mutationFn: async (core: CoreBios) => {
      const filters =
        core.pickExtensions.length > 0
          ? [
              { name: "BIOS files", extensions: core.pickExtensions },
              { name: "All files", extensions: ["*"] },
            ]
          : undefined;
      const path = await open({ multiple: false, directory: false, title: "Choose a BIOS file", filters });
      return typeof path === "string" ? biosCommands.addBiosFile(core.coreId, path) : null;
    },
    onSuccess: (name) => {
      if (name) showNotice(`BIOS added as ${name}`);
    },
    onSettled: () => void queryClient.invalidateQueries({ queryKey: queryKeys.bios }),
    onError: (error) => showError("Cannot add the BIOS file", errorMessage(error)),
  });
}

export function useMemoryCards(gameId: number) {
  return useQuery({
    queryKey: queryKeys.memoryCards(gameId),
    queryFn: () => biosCommands.getMemoryCards(gameId),
  });
}

/** Imports a picked card file into one slot; refused while the game runs. */
export function useImportMemoryCard(gameId: number) {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    mutationFn: ({ slot, path }: { slot: number; path: string }) =>
      biosCommands.importMemoryCard(gameId, slot, path),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: queryKeys.memoryCards(gameId) }),
    onError: (error) => showError("Cannot import the memory card", errorMessage(error)),
  });
}
