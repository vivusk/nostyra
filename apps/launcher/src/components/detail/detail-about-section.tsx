/** About: the user's own description (no description provider in M2). */
export function DetailAboutSection({ description }: { description: string | null }) {
  return (
    <section className="flex flex-col gap-3.5">
      <h2 className="m-0 font-display text-[22px] font-semibold">About</h2>
      {description ? (
        <p className="m-0 max-w-[720px] leading-[1.65] whitespace-pre-line text-text-soft">{description}</p>
      ) : (
        <p className="m-0 text-muted">No description yet. Edit metadata to add one.</p>
      )}
    </section>
  );
}
