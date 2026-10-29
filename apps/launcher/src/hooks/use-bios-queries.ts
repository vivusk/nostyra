// BIOS status (Settings) and memory cards (Detail → Save states).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { biosCommands } from "../api/tauri-commands-bios";
import { errorMessage } from "../api/tauri-commands";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

export function useBiosStatus() {
  return useQuery({ queryKey: queryKeys.bios, queryFn: biosCommands.getBiosStatus });
}

/** Picks a file and copies it into the BIOS folder (the backend validates it). */
export function useAddBiosFile() {
  const queryClient = useQueryClient();
  const { showError, showNotice } = useErrorToast();
  return useMutation({
    mutationFn: async (coreId: string) => {
      const path = await open({ multiple: false, directory: false, title: "Choose a BIOS file" });
      return typeof path === "string" ? biosCommands.addBiosFile(coreId, path) : null;
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
