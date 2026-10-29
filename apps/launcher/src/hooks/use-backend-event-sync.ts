// Subscribes once (in AppShell) to every backend event and folds it into the
// TanStack Query cache, so components only read queries.
import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import {
  EVENTS,
  type ArtworkFinishedEvent,
  type ArtworkProgressEvent,
  type ArtworkUpdatedEvent,
  type CoreDownloadFinishedEvent,
  type CoreDownloadProgressEvent,
  type ScanFinishedEvent,
  type ScanProgressEvent,
  type SessionEndedEvent,
  type SessionEvent,
} from "../api/tauri-events";
import { queryKeys, type GameSessionState } from "../api/query-keys";
import { useErrorToast } from "../components/error-toast";
import { refreshAccountViews } from "./use-achievements-queries";
import { useCreateBugReport } from "./use-bug-report";

export function useBackendEventSync() {
  const queryClient = useQueryClient();
  const { showError, showNotice } = useErrorToast();
  // `mutate` is stable across renders: safe as an effect dependency.
  const { mutate: createBugReport } = useCreateBugReport();

  useEffect(() => {
    const refreshLibrary = () => {
      for (const key of [queryKeys.gamesRoot, queryKeys.summary, queryKeys.setup, queryKeys.home, queryKeys.genres]) {
        void queryClient.invalidateQueries({ queryKey: key });
      }
    };
    // Covers arrive one game at a time: refresh lists at most every 500 ms.
    let artworkRefresh: ReturnType<typeof setTimeout> | undefined;
    const refreshArtworkLists = () => {
      if (artworkRefresh !== undefined) return;
      artworkRefresh = setTimeout(() => {
        artworkRefresh = undefined;
        void queryClient.invalidateQueries({ queryKey: queryKeys.gamesRoot });
        void queryClient.invalidateQueries({ queryKey: queryKeys.home });
      }, 500);
    };
    const setSession = (value: GameSessionState | null) =>
      queryClient.setQueryData(queryKeys.session, value);

    const subscriptions: Promise<UnlistenFn>[] = [
      listen<ScanProgressEvent>(EVENTS.scanProgress, (e) => {
        queryClient.setQueryData(queryKeys.scanProgress, e.payload);
      }),
      listen<ScanFinishedEvent>(EVENTS.scanFinished, (e) => {
        queryClient.setQueryData(queryKeys.scanProgress, null);
        refreshLibrary();
        if (e.payload.error) showError("Library scan failed", e.payload.error);
        const notAdded = e.payload.failedFiles;
        queryClient.setQueryData(queryKeys.scanProblems, e.payload.failures);
        if (notAdded > 0) {
          showNotice(`${notAdded} ${notAdded === 1 ? "file" : "files"} not added`, {
            label: "Details",
            onClick: () => queryClient.setQueryData(queryKeys.scanProblemsOpen, true),
          });
        }
        const moved = e.payload.relinked;
        if (moved > 0) {
          showNotice(`${moved} moved or renamed ${moved === 1 ? "game kept its" : "games kept their"} history`);
        }
      }),
      listen<CoreDownloadProgressEvent>(EVENTS.coreDownloadProgress, (e) => {
        queryClient.setQueryData(queryKeys.coreDownload, e.payload);
      }),
      listen<CoreDownloadFinishedEvent>(EVENTS.coreDownloadFinished, (e) => {
        queryClient.setQueryData(queryKeys.coreDownload, null);
        void queryClient.invalidateQueries({ queryKey: queryKeys.coreStatus });
        void queryClient.invalidateQueries({ queryKey: queryKeys.setup });
        if (e.payload.error) showError("Core download failed", e.payload.error);
      }),
      listen<SessionEvent>(EVENTS.sessionStarted, (e) => {
        setSession({ gameId: e.payload.gameId, phase: "playing" });
      }),
      listen<SessionEvent>(EVENTS.sessionLaunchTimeout, (e) => {
        // Stop showing "Launching…"; the session still ends on process exit.
        setSession({ gameId: e.payload.gameId, phase: "playing" });
      }),
      listen<SessionEndedEvent>(EVENTS.sessionEnded, (e) => {
        setSession(null);
        refreshLibrary();
        void queryClient.invalidateQueries({ queryKey: queryKeys.game(e.payload.gameId) });
        // The runner may have saved states or (pause menu) gameplay settings.
        void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(e.payload.gameId) });
        // A clean exit writes a new resume point (auto.*).
        void queryClient.invalidateQueries({ queryKey: queryKeys.resumeInfo(e.payload.gameId) });
        void queryClient.invalidateQueries({ queryKey: queryKeys.gameplayConfigRoot });
        void queryClient.invalidateQueries({ queryKey: queryKeys.gameAchievements(e.payload.gameId) });
        const error = e.payload.error;
        // Game errors offer a bug-report zip (logs of this session included).
        if (error) {
          showError(error.message, error.detail, { label: "Create bug report", onClick: () => createBugReport() });
        }
      }),
      listen<SessionEvent>(EVENTS.achievementsChanged, (e) => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.gameAchievements(e.payload.gameId) });
      }),
      listen(EVENTS.raLoggedOut, () => {
        refreshAccountViews(queryClient);
        showNotice("RetroAchievements login expired. Log in again in Settings.");
      }),
      listen<ArtworkProgressEvent>(EVENTS.artworkProgress, (e) => {
        queryClient.setQueryData(queryKeys.artworkProgress, e.payload);
      }),
      listen<ArtworkUpdatedEvent>(EVENTS.artworkUpdated, (e) => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.game(e.payload.gameId) });
        refreshArtworkLists();
      }),
      listen<ArtworkFinishedEvent>(EVENTS.artworkFinished, (e) => {
        queryClient.setQueryData(queryKeys.artworkProgress, null);
        void queryClient.invalidateQueries({ queryKey: queryKeys.artworkCacheStats });
        void queryClient.invalidateQueries({ queryKey: queryKeys.setup });
        // Offline / dead network ends quietly (transient > 0, no error).
        if (e.payload.error) showError("Artwork fetch failed", e.payload.error);
      }),
    ];
    return () => {
      clearTimeout(artworkRefresh);
      for (const subscription of subscriptions) {
        void subscription.then((unlisten) => unlisten());
      }
    };
  }, [queryClient, showError, showNotice, createBugReport]);
}
