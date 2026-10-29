import { useRef, type KeyboardEvent } from "react";

export const DETAIL_TABS = ["Overview", "Save states", "Settings", "Achievements"] as const;
export type DetailTab = (typeof DETAIL_TABS)[number];
/** id of the single tab panel the tabs control. */
export const detailTabPanelId = "detail-tab-panel";

interface DetailTabsProps {
  value: DetailTab;
  onChange: (tab: DetailTab) => void;
}

/** ARIA tablist: Left/Right (and Home/End) move between tabs; the active tab has a
 *  3 px accent underline (mockup). Panels use `detailTabPanelId`. */
export function DetailTabs({ value, onChange }: DetailTabsProps) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const current = DETAIL_TABS.indexOf(value);
    const last = DETAIL_TABS.length - 1;
    const next =
      e.key === "ArrowRight" ? (current + 1) % DETAIL_TABS.length
      : e.key === "ArrowLeft" ? (current + last) % DETAIL_TABS.length
      : e.key === "Home" ? 0
      : e.key === "End" ? last
      : null;
    if (next === null) return;
    e.preventDefault();
    onChange(DETAIL_TABS[next]);
    refs.current[next]?.focus();
  };

  return (
    <div role="tablist" aria-label="Game sections" onKeyDown={onKeyDown} className="flex flex-wrap gap-8 border-b border-[#22222A] px-10">
      {DETAIL_TABS.map((tab, index) => {
        const active = tab === value;
        return (
          <button
            key={tab}
            ref={(el) => {
              refs.current[index] = el;
            }}
            type="button"
            role="tab"
            id={`detail-tab-${index}`}
            aria-selected={active}
            aria-controls={detailTabPanelId}
            tabIndex={active ? 0 : -1}
            onClick={() => onChange(tab)}
            className={`h-12 cursor-pointer border-0 bg-transparent px-1 font-body text-[15px] font-bold ${
              active ? "text-white shadow-[inset_0_-3px_0_var(--color-accent-strong)]" : "text-muted hover:text-white"
            }`}
          >
            {tab}
          </button>
        );
      })}
    </div>
  );
}
