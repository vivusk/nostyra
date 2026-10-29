// Navigation context + hooks (provider in navigation-provider.tsx).
import { createContext, useContext, type RefObject } from "react";
import type { LibraryViewState, RootPage, Route } from "./route-types";

export interface NavigationValue {
  route: Route;
  /** Previous route (Detail's back pill says "Home" or "Library"). */
  previous: Route | null;
  canGoBack: boolean;
  /** Sidebar pages replace the stack root; Detail is pushed on top. */
  navigate: (route: Route) => void;
  back: () => void;
  library: LibraryViewState;
  updateLibrary: (patch: Partial<LibraryViewState>) => void;
  /** The scrolling `<main>` (virtual grid scroll element, scroll restore). */
  scrollRef: RefObject<HTMLElement | null>;
}

export const NavigationContext = createContext<NavigationValue | null>(null);

export function useNavigation(): NavigationValue {
  const value = useContext(NavigationContext);
  if (!value) throw new Error("useNavigation outside NavigationProvider");
  return value;
}

/** Root page a route belongs to (Detail highlights nothing in the sidebar). */
export function rootPageOf(route: Route): RootPage | null {
  return route.page === "detail" ? null : route.page;
}
