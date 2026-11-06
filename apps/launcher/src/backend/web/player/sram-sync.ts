// SRAM write-back of one web play session: the player sends the core's SRAM (and
// mGBA's RTC) whenever EmulatorJS saves it — on our 30 s / tab-hidden flush, in its
// own menu, and at exit — and only bytes whose CRC changed are written, atomically,
// to the game's save folder (`sram-rule.ts` picks save.srm or save.web.srm). A busy
// file (phase 00 D13) is retried after 1 s and 5 s, then at the next flush; bytes
// still unwritten when the session ends are parked (pending-saves.ts), never lost.
import type { DataDir } from "../fs/data-dir";
import { crc32Update } from "./crc32-stream";
import type { DataFileNames } from "./game-data-paths";
import { parkPendingSave, removePendingSave } from "./pending-saves";
import { sramToWrite, type SramFolderFacts } from "./sram-rule";

const RETRY_DELAYS_MS = [1_000, 5_000];

type Kind = "sram" | "rtc";

const crcOf = (bytes: Uint8Array | null) => (bytes ? crc32Update(0, bytes) : null);

export class SramSync {
  private latest: Record<Kind, Uint8Array | null> = { sram: null, rtc: null };
  private written: Record<Kind, number | null>;
  private queue: Promise<void> = Promise.resolve();
  private retries = 0;
  private retryTimer: ReturnType<typeof setTimeout> | undefined;
  private enabled = true;

  constructor(
    private readonly data: DataDir,
    private readonly dir: string,
    private readonly names: DataFileNames,
    private readonly facts: SramFolderFacts,
    loaded: Record<Kind, Uint8Array | null>,
  ) {
    this.written = { sram: crcOf(loaded.sram), rtc: crcOf(loaded.rtc) };
    this.latest = { ...loaded };
  }

  /** CRC of the game's SRAM now (state sidecars record it); null = no SRAM. */
  sramCrc32(): number | null {
    return crcOf(this.latest.sram);
  }

  /** Stops writing for this session (the player's save path was not the expected
   *  one: its bytes may belong to another file). */
  disable(): void {
    this.enabled = false;
    clearTimeout(this.retryTimer);
  }

  /** New bytes from the player; written when they differ from the file's. */
  accept(sram: Uint8Array | null, rtc: Uint8Array | null): void {
    // A zero-length buffer means "no battery RAM": never create an empty file.
    if (sram?.length) this.latest.sram = sram;
    if (rtc?.length) this.latest.rtc = rtc;
    this.retries = 0;
    this.schedule();
  }

  private schedule(): Promise<void> {
    this.queue = this.queue.then(() => this.writeDirty().catch((error) => this.onWriteError(error)));
    return this.queue;
  }

  /** Data folder path the bytes of `kind` go to now. */
  private pathOf(kind: Kind, bytes: Uint8Array): string {
    const name = kind === "rtc" ? this.names.rtc : sramToWrite(this.facts, bytes.length) === "webSram" ? this.names.webSram : this.names.sram;
    return `${this.dir}/${name}`;
  }

  private dirty(kind: Kind): Uint8Array | null {
    const bytes = this.latest[kind];
    return this.enabled && bytes && crcOf(bytes) !== this.written[kind] ? bytes : null;
  }

  private async writeDirty(): Promise<void> {
    for (const kind of ["sram", "rtc"] as const) {
      const bytes = this.dirty(kind);
      if (!bytes) continue;
      const path = this.pathOf(kind, bytes);
      await this.data.writeFile(path, bytes);
      this.written[kind] = crcOf(bytes);
      // An older parked copy of this file must not beat these bytes at a retry.
      await removePendingSave(path).catch((error) => console.warn(`stale pending save ${path} not removed`, error));
      if (path.endsWith(`/${this.names.webSram}`)) this.facts.webSramExists = true;
      else if (kind === "sram") this.facts.sramLength = bytes.length;
    }
  }

  private onWriteError(error: unknown): void {
    console.warn("SRAM write to the Data folder failed; kept in memory", error);
    const delay = RETRY_DELAYS_MS[this.retries];
    if (delay === undefined) return; // the next flush tries again
    this.retries += 1;
    clearTimeout(this.retryTimer);
    this.retryTimer = setTimeout(() => void this.schedule(), delay);
  }

  /** Last write at the end of the session; whatever still fails is parked for a
   *  later retry. Resolves to the parked Data folder paths. */
  async finish(): Promise<string[]> {
    clearTimeout(this.retryTimer);
    await this.schedule();
    const parked: string[] = [];
    for (const kind of ["sram", "rtc"] as const) {
      const bytes = this.dirty(kind);
      if (!bytes) continue;
      const path = this.pathOf(kind, bytes);
      await parkPendingSave(path, bytes);
      parked.push(path);
    }
    this.enabled = false;
    return parked;
  }
}
