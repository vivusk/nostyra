// One web game session (desktop: the runner process + `finish_session`): opens the
// player, emits `session-started` / `session-launch-timeout` / `session-ended` with
// the desktop payloads, keeps SRAM in sync (every 30 s and when the tab is hidden),
// routes EmulatorJS's Save / Load buttons through the slot picker, and on exit
// writes the resume point (`resume.auto_save`), the last SRAM and the play time.
import type { SessionEndedEvent } from "../../../api/tauri-events";
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";
import { emit } from "../web-event-bus";
import { openPlayerFrame, type PlayerFrame } from "./player-frame";
import { playerSavePaths, type FromPlayer } from "./player-messages";
import type { PlayLaunch } from "./play-launch";
import { WEB_EVENTS } from "../../../api/web-commands";
import { recordSession } from "./session-recorder";
import { closeSlotPicker, pickSlot } from "./slot-picker";
import { SramSync } from "./sram-sync";
import { readWebState, writeWebState } from "./web-state-slots";

const FLUSH_INTERVAL_MS = 30_000;
/** No "started" by then: the UI stops showing "Launching…" (desktop event). */
const LAUNCH_TIMEOUT_MS = 60_000;
/** The player did not confirm the exit: close it anyway (SRAM already synced). */
const EXIT_TIMEOUT_MS = 10_000;

type SessionError = NonNullable<SessionEndedEvent["error"]>;

const slotLabel = (slot: StateSlotId) => (slot === "auto" ? "the resume point" : `slot ${slot}`);

export class PlaySession {
  private readonly frame: PlayerFrame;
  private readonly sram: SramSync;
  private startedAtMs: number | null = null;
  private exiting = false;
  private finished = false;
  private autoState: { state: Uint8Array; screenshot: Uint8Array | null } | null = null;
  private readonly timers: ReturnType<typeof setTimeout>[] = [];
  private readonly onHidden = () => document.visibilityState === "hidden" && this.frame.send({ type: "flush" });

  constructor(private readonly launch: PlayLaunch, private readonly onEnd: () => void) {
    this.sram = new SramSync(launch.data, launch.savesDir, launch.names, launch.facts, launch.loaded);
    this.frame = openPlayerFrame(launch.boot, (message) => void this.onMessage(message));
    this.timers.push(setTimeout(() => this.startedAtMs ?? emit("session-launch-timeout", { gameId: launch.gameId }), LAUNCH_TIMEOUT_MS));
  }

  get gameId(): number {
    return this.launch.gameId;
  }

  /** Exit button: resume point (when on), last SRAM, then the player closes. */
  exit(): void {
    if (this.exiting) return;
    this.exiting = true;
    closeSlotPicker();
    this.frame.send({ type: "exit", autoState: this.launch.autoSave && this.startedAtMs !== null });
    this.timers.push(setTimeout(() => void this.finish(null), EXIT_TIMEOUT_MS));
  }

  private async onMessage(message: FromPlayer): Promise<void> {
    switch (message.type) {
      case "started":
        return this.onStarted(message.data.saveFilePath);
      case "sram":
        return this.sram.accept(message.data.sram, message.data.rtc);
      case "save-state-request":
        return this.saveState(message.data.state, message.data.screenshot);
      case "load-state-request":
        return this.loadState();
      case "auto-state":
        this.autoState = message.data;
        return;
      case "error":
        console.error("player:", message.data.message);
        if (message.data.fatal && this.startedAtMs === null) return this.finish(this.startError(message.data.message));
        return;
      case "exited":
        return this.finish(null);
      case "exit-request":
        return this.exit();
    }
  }

  private onStarted(saveFilePath: string): void {
    this.startedAtMs = Date.now();
    const expected = `/data/saves/${playerSavePaths(this.launch.saveDir).sram}`;
    if (saveFilePath !== expected) {
      // The bytes would not be this game's save: never write them (phase 00 D4).
      console.error(`player saves to ${saveFilePath}, expected ${expected}: SRAM write-back off`);
      this.sram.disable();
      this.frame.send({ type: "message", text: "Saving is off for this session (unexpected save file)." });
    }
    this.timers.push(setInterval(() => this.frame.send({ type: "flush" }), FLUSH_INTERVAL_MS));
    document.addEventListener("visibilitychange", this.onHidden);
    emit("session-started", { gameId: this.gameId });
  }

  private async saveState(state: Uint8Array, screenshot: Uint8Array | null): Promise<void> {
    if (this.exiting) return;
    this.frame.send({ type: "pause" });
    const slot = await pickSlot(this.gameId, "save");
    try {
      if (slot === null || this.exiting) return;
      await writeWebState(this.launch.data, this.launch.savesDir, slot, {
        ejsCore: this.launch.ejsCore,
        state,
        screenshot,
        sramCrc32: this.sram.sramCrc32(),
        patchCrc32: this.launch.patchCrc32,
      });
      this.frame.send({ type: "message", text: `State saved to ${slotLabel(slot)}` });
    } catch (error) {
      console.error("state not saved", error);
      this.frame.send({ type: "message", text: "Could not save the state" });
    } finally {
      this.frame.send({ type: "resume" });
    }
  }

  private async loadState(): Promise<void> {
    if (this.exiting) return;
    this.frame.send({ type: "pause" });
    const slot = await pickSlot(this.gameId, "load");
    try {
      if (slot === null || this.exiting) return;
      const read = await readWebState(this.launch.data, this.launch.savesDir, slot, this.launch.ejsCore);
      if ("refusal" in read) return this.frame.send({ type: "message", text: read.refusal });
      this.frame.send({ type: "load-state", state: read.bytes });
      this.frame.send({ type: "message", text: `Loaded ${slotLabel(slot)}` });
    } finally {
      this.frame.send({ type: "resume" });
    }
  }

  private startError(detail: string): SessionError {
    const disc = this.launch.system.media === "disc";
    return {
      kind: "core",
      message: `${this.launch.system.displayName} game could not start`,
      detail: disc
        ? `${detail}\nLarge disc images need several GB of browser memory; a CHD copy of the disc needs much less.`
        : detail,
    };
  }

  private async finish(error: SessionError | null): Promise<void> {
    if (this.finished) return;
    this.finished = true;
    for (const timer of this.timers) clearTimeout(timer);
    document.removeEventListener("visibilitychange", this.onHidden);
    closeSlotPicker();
    if (this.autoState && !error) {
      await writeWebState(this.launch.data, this.launch.savesDir, "auto", {
        ejsCore: this.launch.ejsCore,
        ...this.autoState,
        sramCrc32: this.sram.sramCrc32(),
        patchCrc32: this.launch.patchCrc32,
      }).catch((failure) => console.error("resume point not written", failure));
    }
    const parked = await this.sram.finish().catch((failure) => {
      console.error("SRAM could not be parked", failure);
      return [] as string[];
    });
    this.frame.remove();
    if (this.startedAtMs !== null) {
      const { gameId, romPath, sha1 } = this.launch;
      await recordSession({ gameId, romPath, sha1, startedAtMs: this.startedAtMs, endedAtMs: Date.now() }).catch(
        (failure) => console.error("play time not recorded", failure),
      );
    }
    this.onEnd();
    if (parked.length > 0) emit(WEB_EVENTS.pendingSaves, { paths: parked });
    emit<SessionEndedEvent>("session-ended", { gameId: this.gameId, error });
  }
}
