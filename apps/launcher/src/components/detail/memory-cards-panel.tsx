import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { MemoryCardImport } from "../../api/tauri-commands-bios";
import { useImportMemoryCard, useMemoryCards } from "../../hooks/use-bios-queries";
import { ConfirmDialog } from "../confirm-dialog";

/** Detail → Save states → Memory cards (only for games whose core keeps cards):
 *  each slot's card file and save count, "Import card…" from another emulator
 *  (ePSXe .mcr, .mcd, .srm, .gme, .vgs). The current card is backed up first. */
export function MemoryCardsPanel({ gameId, running, missing }: { gameId: number; running: boolean; missing: boolean }) {
  const { data: cards = [] } = useMemoryCards(gameId);
  const importCard = useImportMemoryCard(gameId);
  const [pending, setPending] = useState<{ slot: number; path: string } | null>(null);
  const [result, setResult] = useState<{ slot: number; import: MemoryCardImport } | null>(null);
  if (cards.length === 0) return null;

  const pick = async (slot: number) => {
    const path = await open({
      multiple: false,
      directory: false,
      title: `Choose a memory card for slot ${slot}`,
      filters: [{ name: "Memory cards", extensions: ["mcr", "mcd", "srm", "gme", "vgs", "mem", "bin"] }],
    });
    if (typeof path === "string") setPending({ slot, path });
  };

  return (
    <section aria-labelledby="memory-cards-title" className="flex flex-col gap-3">
      <h3 id="memory-cards-title" className="m-0 font-display text-lg font-semibold">
        Memory cards
      </h3>
      <ul className="m-0 flex max-w-[720px] list-none flex-col gap-2 p-0">
        {cards.map((card) => (
          <li key={card.slot} className="flex flex-wrap items-center gap-3 rounded-control bg-surface-raised px-4 py-2 text-sm">
            <span className="flex-1">
              <b>Slot {card.slot}</b>{" "}
              <span className="text-text-soft">
                {card.unreadable
                  ? "· not a valid memory card (import one to replace it)"
                  : card.saves === null
                    ? "· no card yet (created on first play)"
                    : `· ${card.saves} ${card.saves === 1 ? "save" : "saves"}`}
              </span>
            </span>
            <button
              type="button"
              disabled={running || missing || importCard.isPending}
              title={
                running ? "Close the game first: it is running right now"
                : missing ? "The ROM file is missing. Rescan your library."
                : undefined
              }
              onClick={() => void pick(card.slot)}
              className="h-9 cursor-pointer rounded-control border border-border-strong bg-transparent px-3 font-body text-xs font-bold text-text-soft hover:text-white disabled:cursor-default disabled:opacity-50"
            >
              Import card…
            </button>
          </li>
        ))}
      </ul>
      {result && (
        <p role="status" className="m-0 text-sm text-text-soft">
          Slot {result.slot}: card imported ({result.import.saves} {result.import.saves === 1 ? "save" : "saves"}).
          {result.import.serialFound === false && " None of its saves is for this game — check that you picked the right file."}
        </p>
      )}
      {pending && (
        <ConfirmDialog
          title={`Import into slot ${pending.slot}?`}
          confirmLabel="Import"
          onConfirm={() =>
            importCard.mutate(pending, {
              onSuccess: (done) => setResult({ slot: pending.slot, import: done }),
            })
          }
          onClose={() => setPending(null)}
        >
          The card replaces this game's slot {pending.slot} card. The current card is kept as a backup file in the
          game's save folder. Loading an older save state can bring back the card stored in that state.
        </ConfirmDialog>
      )}
    </section>
  );
}
