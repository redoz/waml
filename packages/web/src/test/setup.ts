import { initWasm } from "@waml/wasm";

// The bundle-as-truth store derives its model via the WASM core synchronously, so
// every test that imports the store singleton (directly or via `model.svelte` /
// `bootstrap`) needs the wasm module ready before its top-level imports evaluate.
// Setup files run before the test module is imported, so init it here once.
await initWasm();

// jsdom has no ResizeObserver; @xyflow/svelte needs one to mount its panes.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(globalThis as unknown as { ResizeObserver: typeof ResizeObserverStub }).ResizeObserver =
  ResizeObserverStub;

// jsdom has no window.matchMedia; @xyflow/svelte 1.x reads a MediaQuery
// (Svelte's reactive matchMedia wrapper) while constructing its store, even
// for a blank canvas with no components using media queries directly.
if (!window.matchMedia) {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}
