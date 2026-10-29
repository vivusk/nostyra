// Gameplay settings and controls: effective + layer values (global defaults or one
// game), single-key writes, binding assign (steal) and controls reset. Session end
// invalidates these (pause-menu saves).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  gameplayCommands,
  type BindingKey,
  type GameplayConfig,
  type GameplayConfigKey,
  type GameplayConfigScope,
  type GameplayConfigView,
} from "../api/tauri-commands-gameplay";
import { errorMessage } from "../api/tauri-commands";
import { inputCommands, type InputDevice } from "../api/tauri-commands-input";
import { queryKeys } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";

/** Config for one game (Detail Settings tab) or the global defaults (`null`, Settings page). */
export function useGameplayConfig(gameId: number | null) {
  return useQuery({
    queryKey: queryKeys.gameplayConfig(gameId),
    queryFn: () => gameplayCommands.getGameplayConfig(gameId ?? undefined),
  });
}

export interface GameplayConfigEdit<K extends GameplayConfigKey = GameplayConfigKey> {
  scope: GameplayConfigScope;
  key: K;
  /** `null` = "Use default" (removes the key from the layer). */
  value: GameplayConfig[K] | null;
}

/** The cached view a write to `scope` edits (global → the defaults view). */
const scopeQueryKey = (scope: GameplayConfigScope) => queryKeys.gameplayConfig(scope === "global" ? null : scope.gameId);

/** `view` with `key` set (or cleared) in the edited layer. A cleared key keeps the old
 *  effective value until the refetch brings the real fallback. */
function withEdit(view: GameplayConfigView, { scope, key, value }: GameplayConfigEdit): GameplayConfigView {
  const layerName = scope === "global" ? "global" : "game";
  const layer: Partial<GameplayConfig> = { ...view[layerName] };
  if (value === null) delete layer[key];
  else Object.assign(layer, { [key]: value });
  const resolved = value === null ? view.resolved : { ...view.resolved, [key]: value };
  return { ...view, [layerName]: layer, resolved };
}

/** Writes one key optimistically (rolled back + toast on error); every gameplay-config
 *  query then refetches (a global change shows through games). Writes run one at a
 *  time (mutation scope) because each one rewrites the whole layer file. */
export function useSetGameplayConfigValue() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    scope: { id: "gameplay-config-write" },
    mutationFn: ({ scope, key, value }: GameplayConfigEdit) =>
      gameplayCommands.setGameplayConfigValue(scope, key, value),
    onMutate: async (edit) => {
      const key = scopeQueryKey(edit.scope);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<GameplayConfigView>(key);
      if (previous) queryClient.setQueryData(key, withEdit(previous, edit));
      return { key, previous };
    },
    onError: (error, _edit, context) => {
      if (context?.previous) queryClient.setQueryData(context.key, context.previous);
      showError("Cannot save the setting", errorMessage(error));
    },
    onSettled: () => invalidateConfig(queryClient),
  });
}

type QueryClient = ReturnType<typeof useQueryClient>;

/** Every config view plus resume info (it carries the effective `resume.mode`). */
function invalidateConfig(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.gameplayConfigRoot }),
    queryClient.invalidateQueries({ queryKey: queryKeys.resumeInfoRoot }),
  ]);
}

export interface AssignBindingRequest {
  scope: GameplayConfigScope;
  device: InputDevice;
  /** Binding key, e.g. "input_keyboard.a". */
  target: BindingKey;
  input: string;
}

/** Binds an input with steal (backend computes and writes it in one go). Resolves
 *  with `removedFrom` for the "Z removed from Y" toast; shares the write queue. */
export function useAssignBinding() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    scope: { id: "gameplay-config-write" },
    mutationFn: ({ scope, device, target, input }: AssignBindingRequest) =>
      inputCommands.assignBinding(scope, device, target, input),
    onError: (error) => showError("Cannot assign the input", errorMessage(error)),
    onSettled: () => invalidateConfig(queryClient),
  });
}

/** Chip remove / Clear (`[]`) / Use default (`null`) for one binding key. */
export function useSetBindingValue() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    scope: { id: "gameplay-config-write" },
    mutationFn: ({ scope, key, value }: { scope: GameplayConfigScope; key: BindingKey; value: string[] | null }) =>
      inputCommands.setBindingValue(scope, key, value),
    onError: (error) => showError("Cannot save the control", errorMessage(error)),
    onSettled: () => invalidateConfig(queryClient),
  });
}

/** Removes every binding key of the scope (gameplay settings stay). */
export function useResetControls() {
  const queryClient = useQueryClient();
  const { showError } = useErrorToast();
  return useMutation({
    scope: { id: "gameplay-config-write" },
    mutationFn: (scope: GameplayConfigScope) => inputCommands.resetControls(scope),
    onError: (error) => showError("Cannot reset the controls", errorMessage(error)),
    onSettled: () => invalidateConfig(queryClient),
  });
}
