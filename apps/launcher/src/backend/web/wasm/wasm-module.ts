// The web edition's Rust module (crates/web-bindings, built by
// scripts/build-web-wasm.*), initialised once per realm: the page (catalog, metadata
// reset) and the scan worker each load their own instance.
import init, * as wasm from "../wasm-pkg/nostyra_web.js";

export type NostyraWasm = typeof wasm;

let ready: Promise<NostyraWasm> | null = null;

/** The initialised module (first call fetches and compiles the .wasm). */
export function loadWasm(): Promise<NostyraWasm> {
  ready ??= init().then(() => wasm);
  return ready;
}
