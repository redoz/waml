import { test, expect, beforeEach, vi } from "vitest";
import { persistGraph } from "@uaml/core/state/persist";
import { createModelStore } from "@uaml/core/state/model";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});

test("first-ever visit: empty store, isFirstVisit true", async () => {
  const mod = await import("./bootstrap");
  expect(mod.isFirstVisit).toBe(true);
  expect(mod.sharedModelName).toBeNull();
  expect(mod.store.get().nodes.length).toBe(0);
});

test("rehydrates a persisted graph and is not a first visit", async () => {
  const seed = createModelStore();
  seed.addNode({ x: 10, y: 20 });
  persistGraph(seed.get());

  const mod = await import("./bootstrap");
  expect(mod.isFirstVisit).toBe(false);
  expect(mod.store.get().nodes.length).toBe(1);
});
