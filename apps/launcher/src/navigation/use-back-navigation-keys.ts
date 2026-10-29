// Back shortcuts: Alt+Left, mouse "back" button (button 3), Escape on Detail
// (unless a dialog is open — the dialog handles Escape itself).
import { useEffect } from "react";
import { useNavigation } from "./navigation-context";

export function useBackNavigationKeys() {
  const { back, canGoBack, route } = useNavigation();

  useEffect(() => {
    if (!canGoBack) return;
    const onKey = (e: KeyboardEvent) => {
      const escapeOnDetail = e.key === "Escape" && route.page === "detail" && !document.querySelector("dialog[open]");
      if ((e.altKey && e.key === "ArrowLeft") || escapeOnDetail) {
        e.preventDefault();
        back();
      }
    };
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3) {
        e.preventDefault();
        back();
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mouseup", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mouseup", onMouse);
    };
  }, [back, canGoBack, route.page]);
}
