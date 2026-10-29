import type { ReactNode } from "react";

interface HomeSectionProps {
  id: string;
  title: string;
  onSeeAll: () => void;
  children: ReactNode;
}

/** Home row: 22 px Sora heading, "See all" link-button on the right. */
export function HomeSection({ id, title, onSeeAll, children }: HomeSectionProps) {
  return (
    <section aria-labelledby={id} className="flex flex-col gap-4">
      <div className="flex items-baseline gap-3">
        <h2 id={id} className="m-0 font-display text-[22px] font-semibold">
          {title}
        </h2>
        <button
          type="button"
          onClick={onSeeAll}
          className="ml-auto cursor-pointer border-0 bg-transparent font-body text-sm text-link hover:text-[#DAD5FF]"
        >
          See all
        </button>
      </div>
      {children}
    </section>
  );
}
