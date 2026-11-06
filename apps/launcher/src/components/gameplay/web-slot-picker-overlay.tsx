import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "../../backend";
import { queryKeys } from "../../api/query-keys";
import type { SaveStateSlot, StateSlotId } from "../../api/tauri-commands-gameplay";
import { WEB_EVENTS, webCommands, type WebSlotPickerRequest } from "../../api/web-commands";
import { useSaveStates } from "../../hooks/use-save-state-queries";
import { assetUrl, useResolvedAsset } from "../../lib/asset-url";
import { relativeTime } from "../../lib/relative-time";
import { saveSlotLabel } from "../../lib/save-slot-label";

const SLOTS: StateSlotId[] = ["auto", 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

/** Web edition: the slot picker EmulatorJS's in-game Save / Load buttons open
 *  (backend `web-slot-picker` event): the resume point and slots 0–9 with their
 *  thumbnails. Saving goes to a numbered slot (the resume point is written when you
 *  quit); loading offers the used, loadable slots. Escape / Cancel = no change. */
export function WebSlotPickerOverlay() {
  const queryClient = useQueryClient();
  const [request, setRequest] = useState<WebSlotPickerRequest>(null);
  useEffect(() => {
    const unlisten = listen<WebSlotPickerRequest>(WEB_EVENTS.slotPicker, (event) => {
      if (event.payload) void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(event.payload.gameId) });
      setRequest(event.payload);
    });
    return () => void unlisten.then((stop) => stop());
  }, [queryClient]);
  if (!request) return null;
  const answer = (slot: StateSlotId | null) => {
    setRequest(null);
    void webCommands.chooseSlot(slot);
    if (slot !== null) void queryClient.invalidateQueries({ queryKey: queryKeys.saveStates(request.gameId) });
  };
  return <SlotPickerDialog request={request} onAnswer={answer} />;
}

function SlotPickerDialog({ request, onAnswer }: { request: NonNullable<WebSlotPickerRequest>; onAnswer: (slot: StateSlotId | null) => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const { data: used = [] } = useSaveStates(request.gameId);
  useEffect(() => dialogRef.current?.showModal(), []);
  const saving = request.mode === "save";
  return (
    <dialog
      ref={dialogRef}
      onClose={() => onAnswer(null)}
      aria-labelledby="slot-picker-title"
      className="m-auto w-[min(920px,calc(100vw-48px))] rounded-[14px] border border-border-strong bg-surface p-0 text-text backdrop:bg-black/70"
    >
      <div className="flex flex-col gap-5 p-7">
        <h2 id="slot-picker-title" className="m-0 font-display text-[22px] font-semibold">
          {saving ? "Save state to…" : "Load state from…"}
        </h2>
        <ul className="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-3 p-0">
          {SLOTS.map((slot) => {
            const state = used.find((s) => s.slot === slot) ?? null;
            const reason = saving
              ? slot === "auto" ? "Written when you quit" : null
              : !state ? "Empty" : state.incompatibleReason;
            return (
              <li key={String(slot)}>
                <SlotButton slot={slot} state={state} disabledReason={reason} onPick={() => onAnswer(slot)} />
              </li>
            );
          })}
        </ul>
        <div className="flex justify-end">
          <button
            type="button"
            onClick={() => dialogRef.current?.close()}
            className="h-11 cursor-pointer rounded-control border-0 bg-transparent px-4 font-body font-semibold text-text-soft hover:text-white"
          >
            Cancel
          </button>
        </div>
      </div>
    </dialog>
  );
}

function SlotButton({ slot, state, disabledReason, onPick }: { slot: StateSlotId; state: SaveStateSlot | null; disabledReason: string | null; onPick: () => void }) {
  const thumbnail = useResolvedAsset(assetUrl(state?.thumbnailPath ?? null, state?.createdAt));
  const label = saveSlotLabel(slot);
  return (
    <button
      type="button"
      onClick={onPick}
      disabled={disabledReason !== null}
      title={disabledReason ?? undefined}
      className="flex w-full cursor-pointer flex-col overflow-hidden rounded-card border border-border bg-surface-raised p-0 text-left font-body text-text hover:border-accent disabled:cursor-default disabled:opacity-50"
    >
      <span className="flex aspect-[4/3] w-full items-center justify-center bg-black text-xs text-muted">
        {thumbnail ? <img src={thumbnail} alt="" className="size-full object-contain [image-rendering:pixelated]" /> : state ? "No preview" : "Empty"}
      </span>
      <span className="flex items-baseline justify-between gap-2 px-3 py-2 text-sm">
        <b>{label}</b>
        <span className="text-xs text-muted">{state ? relativeTime(state.createdAt) : disabledReason ?? "Empty"}</span>
      </span>
    </button>
  );
}
