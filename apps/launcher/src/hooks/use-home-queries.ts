// Home page data: one backend call (continue playing, recently added, favorites).
import { useQuery } from "@tanstack/react-query";
import { artworkCommands } from "../api/tauri-commands-artwork";
import { queryKeys } from "../api/query-keys";

export function useHomeData() {
  return useQuery({ queryKey: queryKeys.home, queryFn: artworkCommands.getHomeData });
}
