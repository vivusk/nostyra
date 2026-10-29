import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { NavigationContext, type NavigationValue } from "./navigation-context";
import { DEFAULT_LIBRARY_VIEW, type LibraryViewState, type Route } from "./route-types";

/** Holds the route stack and the Library view state for the whole shell. */
export function NavigationProvider({ initial, children }: { initial: Route; children: ReactNode }) {
  const [stack, setStack] = useState<Route[]>([initial]);
  const [library, setLibrary] = useState<LibraryViewState>(DEFAULT_LIBRARY_VIEW);
  const scrollRef = useRef<HTMLElement | null>(null);

  const navigate = useCallback((route: Route) => {
    setStack((current) => {
      if (route.page !== "detail") return [route];
      const top = current[current.length - 1];
      // Re-opening the same game (or another from Detail) replaces the top entry.
      return top.page === "detail" ? [...current.slice(0, -1), route] : [...current, route];
    });
  }, []);
  const back = useCallback(() => setStack((current) => (current.length > 1 ? current.slice(0, -1) : current)), []);
  const updateLibrary = useCallback(
    (patch: Partial<LibraryViewState>) => setLibrary((current) => ({ ...current, ...patch })),
    [],
  );

  const value = useMemo<NavigationValue>(
    () => ({
      route: stack[stack.length - 1],
      previous: stack.length > 1 ? stack[stack.length - 2] : null,
      canGoBack: stack.length > 1,
      navigate,
      back,
      library,
      updateLibrary,
      scrollRef,
    }),
    [stack, navigate, back, library, updateLibrary],
  );
  return <NavigationContext.Provider value={value}>{children}</NavigationContext.Provider>;
}
