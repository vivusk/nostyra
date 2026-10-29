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
import { ErrorToastProvider } from "./components/error-toast";
import { ResumeFlowProvider } from "./components/resume-flow-provider";

const queryClient = new QueryClient();

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("index.html is missing the #root element");
}

createRoot(rootElement).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ErrorToastProvider>
        <ResumeFlowProvider>
          <AppShell />
        </ResumeFlowProvider>
      </ErrorToastProvider>
    </QueryClientProvider>
  </StrictMode>,
);
