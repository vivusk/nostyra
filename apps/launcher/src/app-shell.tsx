import { useLayoutEffect } from "react";
import { LaunchingOverlay } from "./components/launching-overlay";
import { ScanProblemsHost } from "./components/scan-problems-dialog";
import { SidebarNavigation } from "./components/sidebar-navigation";
import { useBackendEventSync } from "./hooks/use-backend-event-sync";
import { useLibrarySummary, useSetupState } from "./hooks/use-library-queries";
import { useLibrarySystems } from "./hooks/use-system-queries";
import { rootPageOf, useNavigation } from "./navigation/navigation-context";
import { NavigationProvider } from "./navigation/navigation-provider";
import { DEFAULT_LIBRARY_VIEW, type RootPage, type Route } from "./navigation/route-types";
import { useBackNavigationKeys } from "./navigation/use-back-navigation-keys";
import { FirstRunSetupPage } from "./pages/first-run-setup-page";
import { GameDetailPage } from "./pages/game-detail-page";
import { HomePage } from "./pages/home-page";
import { LibraryPage } from "./pages/library-page";
import { SettingsPage } from "./pages/settings-page";

/** Landing page once the library is set up. */
const INITIAL_ROUTE: Route = { page: "home" };

/** First-run setup until a library folder exists; then sidebar + routed pages. */
export function AppShell() {
  useBackendEventSync();
  const { data: setup, isPending } = useSetupState();

  if (isPending) return null; // local IPC: resolves within a frame
  if (!setup?.hasFolders) return <FirstRunSetupPage />;
  return (
    <NavigationProvider initial={INITIAL_ROUTE}>
      <ShellLayout />
    </NavigationProvider>
  );
}

function ShellLayout() {
  const { route, navigate, scrollRef, library, updateLibrary } = useNavigation();
  const { data: summary } = useLibrarySummary();
  const { data: systems = [] } = useLibrarySystems();
  const openSystem = (system: string) => {
    updateLibrary({ system, scrollOffset: 0 });
    navigate({ page: "library" });
  };
  // Sidebar "Library" means the whole library: drop system/search/filter/genre and
  // start at the top; sort is a preference and stays.
  const openPage = (page: RootPage) => {
    if (page === "library") {
      updateLibrary({ ...DEFAULT_LIBRARY_VIEW, sort: library.sort });
      scrollRef.current?.scrollTo({ top: 0 }); // already on Library: the grid stays mounted
    }
    navigate({ page });
  };
  useBackNavigationKeys();

  // New page starts at the top; the Library restores its own offset.
  useLayoutEffect(() => {
    if (route.page !== "library" && scrollRef.current) scrollRef.current.scrollTop = 0;
  }, [route, scrollRef]);

  return (
    <div className="flex h-full">
      <SidebarNavigation
        currentPage={rootPageOf(route)}
        onNavigate={openPage}
        gameCount={summary?.total}
        systems={systems.length > 1 ? systems : []}
        activeSystem={library.system}
        onOpenSystem={openSystem}
      />
      <main ref={scrollRef} className="min-w-0 flex-1 overflow-y-auto px-10 pt-8 pb-14">
        <RoutedPage route={route} />
      </main>
      <LaunchingOverlay />
      <ScanProblemsHost />
    </div>
  );
}

function RoutedPage({ route }: { route: Route }) {
  switch (route.page) {
    case "home":
      return <HomePage />;
    case "library":
      return <LibraryPage focusSearch={route.focusSearch} />;
    case "detail":
      return <GameDetailPage key={route.gameId} gameId={route.gameId} />;
    case "settings":
      return <SettingsPage />;
  }
}
