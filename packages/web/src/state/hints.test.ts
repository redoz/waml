import { test, expect, beforeEach, vi } from "vitest";

const KEY = "waml:show-shortcuts";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules(); // re-import so the module re-reads localStorage at init
});

test("defaults to false when nothing is stored", async () => {
  const { hints } = await import("./hints.svelte");
  expect(hints.show).toBe(false);
});

test("initializes from a stored '1'", async () => {
  localStorage.setItem(KEY, "1");
  const { hints } = await import("./hints.svelte");
  expect(hints.show).toBe(true);
});

test("toggle flips the value and persists it", async () => {
  const { hints } = await import("./hints.svelte");
  hints.toggle();
  expect(hints.show).toBe(true);
  expect(localStorage.getItem(KEY)).toBe("1");
  hints.toggle();
  expect(hints.show).toBe(false);
  expect(localStorage.getItem(KEY)).toBe("0");
});

test("setting show persists it", async () => {
  const { hints } = await import("./hints.svelte");
  hints.show = true;
  expect(localStorage.getItem(KEY)).toBe("1");
});
