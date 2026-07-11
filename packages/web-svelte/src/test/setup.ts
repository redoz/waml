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
