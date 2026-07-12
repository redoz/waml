import { test, expect, beforeAll, beforeEach, vi } from "vitest";
import { initWasm } from "@uaml/okf";
import { persistBundle } from "@uaml/core/state/persist";
import { createModelStore } from "@uaml/core/state/model";

beforeAll(async () => {
  await initWasm();
});

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});

// `bootstrap` builds its store synchronously at import (main.ts awaits initWasm
// before loading the app). `vi.resetModules()` gives each test a fresh bootstrap +
// okf instance, so initialize that fresh wasm module before importing bootstrap.
async function loadBootstrap() {
  const okf = await import("@uaml/okf");
  await okf.initWasm();
  return import("./bootstrap");
}

test("first-ever visit: empty store, isFirstVisit true", async () => {
  const mod = await loadBootstrap();
  expect(mod.isFirstVisit).toBe(true);
  expect(mod.sharedModelName).toBeNull();
  expect(mod.store.get().nodes.length).toBe(0);
});

test("rehydrates a persisted bundle and is not a first visit", async () => {
  const seed = createModelStore();
  seed.addNode({ x: 10, y: 20 });
  persistBundle(seed.getBundle());

  const mod = await loadBootstrap();
  expect(mod.isFirstVisit).toBe(false);
  expect(mod.store.get().nodes.length).toBe(1);
});
