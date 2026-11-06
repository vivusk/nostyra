// Hands a walked folder's files to the wasm module under their virtual paths, read on
// demand with `FileReaderSync` (worker only: the Rust identity code is synchronous).
// Every file of the folder is registered, so cue sheets, m3u playlists and disc
// grouping find their siblings by path, exactly like on the desktop.
/// <reference lib="webworker" />
import type { NostyraWasm } from "../wasm/wasm-module";
import { virtualPathOf } from "./scan-paths";
import type { WalkedFile } from "./scan-walk";

const reader = new FileReaderSync();

/** Replaces the registered files with `files` of folder `token`. */
export function registerFolderFiles(wasm: NostyraWasm, token: string, files: readonly WalkedFile[]): void {
  wasm.files_clear();
  for (const { relative, file } of files) {
    wasm.files_add(virtualPathOf(token, relative), file.size, (offset: number, length: number) =>
      new Uint8Array(reader.readAsArrayBuffer(file.slice(offset, offset + length))),
    );
  }
}
