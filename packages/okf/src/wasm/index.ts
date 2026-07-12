// Frontend-facing entry to the Rust UAML core, compiled to wasm and inlined
// (no runtime .wasm fetch). Call `initWasm()` once before any other export.
import init, {
  apply_ops,
  build_bundle,
  build_model,
  fmt,
  init_panic_hook,
  split_bundle,
  validate,
} from "../generated/uaml_wasm.js";
import { wasmBytes } from "../generated/wasm-inline";

let ready: Promise<void> | undefined;

/** Instantiate the inlined wasm exactly once. Safe to await repeatedly. */
export function initWasm(): Promise<void> {
  if (!ready) {
    ready = init({ module_or_path: wasmBytes() }).then(() => {
      init_panic_hook();
    });
  }
  return ready;
}

export { apply_ops, build_bundle, build_model, fmt, split_bundle, validate };
