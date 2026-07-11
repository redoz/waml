import { readable, type Readable } from "svelte/store";
import type { ModelGraph } from "@mc/okf";
import type { ModelStore } from "@mc/core/state/model";
import { store } from "./bootstrap";

// @mc/core's createModelStore().subscribe is a bare-callback external store:
//   subscribe(f: () => void): () => void
// It fires `f` on every change but does NOT pass the value, and does NOT call
// `f` on subscribe. Svelte's store contract requires subscribe(run) to CALL
// `run` with the current value immediately and on every change. Bridge them:
// seed with store.get(), then re-read store.get() on each emit. `$model` in a
// component (or get(model) in a test) then reads the live graph reactively.
export function toModelReadable(s: ModelStore): Readable<ModelGraph> {
  return readable<ModelGraph>(s.get(), (set) => {
    set(s.get()); // guard against a change between module load and first subscribe
    return s.subscribe(() => set(s.get())); // re-read on every change; returns the unsubscribe
  });
}

// Reactive, read-only view of the graph for components (`$model`).
export const model: Readable<ModelGraph> = toModelReadable(store);

// The store itself is the source of truth for mutations (store.updateNode, …).
export { store };
