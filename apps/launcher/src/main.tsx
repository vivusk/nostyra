import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
// Fonts are bundled (offline app): only the weights the mockup uses.
import "@fontsource/sora/500.css";
import "@fontsource/sora/600.css";
import "@fontsource/sora/700.css";
import "@fontsource/dm-sans/400.css";
import "@fontsource/dm-sans/500.css";
import "@fontsource/dm-sans/700.css";
import "./styles.css";
import { AppShell } from "./app-shell";
import { boot } from "./backend";
import { AlreadyOpenScreen } from "./components/already-open-screen";
import { ErrorToastProvider } from "./components/error-toast";
import { ResumeFlowProvider } from "./components/resume-flow-provider";

const queryClient = new QueryClient();

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("index.html is missing the #root element");
}

// Web edition: public/browser-gate.js marks browsers it cannot run in and shows its
// own page; the app does not start there. (MODE is static: the desktop drops the check.)
const unsupported = import.meta.env.MODE === "web" && document.documentElement.hasAttribute("data-unsupported-browser");
if (!unsupported) start(rootElement);

function start(element: HTMLElement) {
  const root = createRoot(element);
  // The web edition runs in one tab at a time (single-tab lock); the desktop is always ready.
  void boot().then((result) =>
    root.render(
      <StrictMode>
        {result === "busy" ? (
          <AlreadyOpenScreen />
        ) : (
          <QueryClientProvider client={queryClient}>
            <ErrorToastProvider>
              <ResumeFlowProvider>
                <AppShell />
              </ResumeFlowProvider>
            </ErrorToastProvider>
          </QueryClientProvider>
        )}
      </StrictMode>,
    ),
  );
}
