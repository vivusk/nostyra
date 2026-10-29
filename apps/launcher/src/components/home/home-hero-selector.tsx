import { assetUrl } from "../../lib/asset-url";
import { heroSubtitle, type HeroEntry } from "../../lib/home-hero-entries";
import { CoverImage } from "../cover-image";

interface HomeHeroSelectorProps {
  entries: HeroEntry[];
  selected: number;
  onSelect: (index: number) => void;
}

/** Epic-style column next to the hero (wraps below it at narrow widths): 60×76 cover
 *  thumb + title + reason; the active row has an accent inset bar. */
export function HomeHeroSelector({ entries, selected, onSelect }: HomeHeroSelectorProps) {
  return (
    <div role="group" aria-label="Featured games" className="flex max-w-[300px] flex-[1_1_240px] flex-col gap-2">
      {entries.map((entry, index) => {
        const active = index === selected;
        return (
          <button
            key={entry.game.id}
            type="button"
            aria-pressed={active}
            onClick={() => onSelect(index)}
            className={`flex min-h-24 cursor-pointer items-center gap-3.5 rounded-card border-0 p-2.5 text-left font-body ${
              active ? "bg-[#22222B] shadow-[inset_3px_0_0_var(--color-accent-strong)]" : "bg-transparent hover:bg-nav-hover"
            }`}
          >
            <CoverImage
              src={assetUrl(entry.game.coverPath, entry.game.coverCheckedAt)}
              title={entry.game.title}
              className="h-[76px] w-[60px] flex-none rounded-lg"
              monogramClassName="text-base"
            />
            <span className="flex min-w-0 flex-col gap-0.5">
              <span className="line-clamp-2 font-bold text-text">{entry.game.title}</span>
              <span className="text-xs text-muted">{heroSubtitle(entry)}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
