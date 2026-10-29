import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

interface ToastMessage {
  id: number;
  title: string;
  detail?: string;
  /** "notice" = neutral information (accent dot), e.g. "Z removed from B". */
  tone: "error" | "notice";
  /** Optional button (e.g. "Details" opening a dialog); clicking it dismisses the toast. */
  action?: ToastAction;
}

export interface ToastAction {
  label: string;
  onClick: () => void;
}

interface ErrorToastApi {
  /** `action`: e.g. "Create bug report" on game errors. */
  showError: (title: string, detail?: string, action?: ToastAction) => void;
  /** Same stack, for non-error feedback that needs no confirmation. */
  showNotice: (title: string, action?: ToastAction) => void;
}

const ErrorToastContext = createContext<ErrorToastApi>({ showError: () => {}, showNotice: () => {} });

/** Errors from commands and game sessions (plus short notices), shown bottom-right. */
export function useErrorToast() {
  return useContext(ErrorToastContext);
}

const AUTO_HIDE_MS = 10_000;

export function ErrorToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const dismiss = useCallback((id: number) => setToasts((all) => all.filter((t) => t.id !== id)), []);
  const push = useCallback((toast: Omit<ToastMessage, "id">) => {
    setToasts((all) => [...all.slice(-2), { ...toast, id: Date.now() + Math.random() }]);
  }, []);
  const showError = useCallback(
    (title: string, detail?: string, action?: ToastAction) => push({ title, detail, tone: "error", action }),
    [push],
  );
  const showNotice = useCallback(
    (title: string, action?: ToastAction) => push({ title, tone: "notice", action }),
    [push],
  );
  const api = useMemo(() => ({ showError, showNotice }), [showError, showNotice]);

  return (
    <ErrorToastContext.Provider value={api}>
      {children}
      {/* No live role on the stack: each toast announces itself (errors assertive, notices polite). */}
      <div className="fixed right-6 bottom-6 z-50 flex w-[380px] flex-col gap-3">
        {toasts.map((toast) => (
          <Toast key={toast.id} toast={toast} onDismiss={dismiss} />
        ))}
      </div>
    </ErrorToastContext.Provider>
  );
}

function Toast({ toast, onDismiss }: { toast: ToastMessage; onDismiss: (id: number) => void }) {
  const [showDetail, setShowDetail] = useState(false);
  useEffect(() => {
    if (showDetail) return; // reading the details: keep it open
    const timer = window.setTimeout(() => onDismiss(toast.id), AUTO_HIDE_MS);
    return () => window.clearTimeout(timer);
  }, [toast.id, onDismiss, showDetail]);

  return (
    <div
      role={toast.tone === "notice" ? "status" : "alert"}
      aria-live={toast.tone === "notice" ? "polite" : "assertive"}
      className="rounded-card border border-border-strong bg-surface-raised p-4 text-sm shadow-2xl"
    >
      <div className="flex items-start gap-3">
        <span aria-hidden="true" className={`mt-1.5 size-2 shrink-0 rounded-full ${toast.tone === "notice" ? "bg-accent" : "bg-warning"}`} />
        <p className="m-0 flex-1 font-medium text-text">{toast.title}</p>
        {toast.action && (
          <button
            type="button"
            onClick={() => {
              toast.action?.onClick();
              onDismiss(toast.id);
            }}
            className="min-h-8 cursor-pointer border-0 bg-transparent p-0 font-body text-xs font-bold text-link"
          >
            {toast.action.label}
          </button>
        )}
        <button
          type="button"
          aria-label="Dismiss"
          onClick={() => onDismiss(toast.id)}
          className="min-h-8 min-w-8 cursor-pointer rounded-control border-0 bg-transparent text-muted hover:text-text"
        >
          ✕
        </button>
      </div>
      {toast.detail && (
        <>
          <button
            type="button"
            onClick={() => setShowDetail((v) => !v)}
            className="mt-1 ml-5 cursor-pointer border-0 bg-transparent p-0 font-body text-xs font-bold text-link"
          >
            {showDetail ? "Hide details" : "Details"}
          </button>
          {showDetail && (
            <pre className="mt-2 max-h-48 overflow-auto rounded-control bg-app-bg p-3 text-xs whitespace-pre-wrap text-muted">
              {toast.detail}
            </pre>
          )}
        </>
      )}
    </div>
  );
}
