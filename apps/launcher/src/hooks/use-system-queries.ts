// The system registry (static for the app's lifetime) and the library's systems.
import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "../api/query-keys";
import { systemCommands, type SystemView } from "../api/tauri-commands-systems";

export function useSystems() {
  return useQuery({ queryKey: queryKeys.systems, queryFn: systemCommands.getSystems, staleTime: Infinity });
}

/** Systems with games (Library filter pills, card badges); refreshed with the summary. */
export function useLibrarySystems() {
  return useQuery({ queryKey: queryKeys.librarySystems, queryFn: systemCommands.getLibrarySystems });
}

/** Short label of a platform id ("SNES"); the id itself while the registry loads. */
export function systemLabel(systems: SystemView[] | undefined, platformId: string): string {
  return systems?.find((s) => s.id === platformId)?.shortLabel ?? platformId.toUpperCase();
}

/** "Super Nintendo (.sfc, .smc) or Nintendo Entertainment System (.nes)". */
export function supportedSystemsText(systems: SystemView[] | undefined): string {
  if (!systems?.length) return "games";
  const parts = systems.map((s) => `${s.displayName} (${s.extensions.map((e) => `.${e}`).join(", ")})`);
  return parts.length === 1 ? parts[0] : `${parts.slice(0, -1).join(", ")} or ${parts[parts.length - 1]}`;
}
