import { useSetFavorite } from "../../hooks/use-game-detail-queries";

/** Heart toggle (optimistic); `aria-pressed` carries the state for screen readers. */
export function FavoriteToggleButton({ gameId, favorite }: { gameId: number; favorite: boolean }) {
  const setFavorite = useSetFavorite();
  return (
    <button
      type="button"
      aria-pressed={favorite}
      aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
      onClick={() => setFavorite.mutate({ gameId, favorite: !favorite })}
      className="flex size-14 cursor-pointer items-center justify-center rounded-card border-0 bg-white/12 text-white hover:bg-white/20"
    >
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill={favorite ? "currentColor" : "none"}
        stroke="currentColor"
        strokeWidth="2"
        aria-hidden="true"
      >
        <path d="M12 21s-7-4.5-9.5-9A5.5 5.5 0 0 1 12 6a5.5 5.5 0 0 1 9.5 6c-2.5 4.5-9.5 9-9.5 9z" />
      </svg>
    </button>
  );
}
