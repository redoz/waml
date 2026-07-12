import { test, expect, beforeEach } from "vitest";
import { loadPersistedBundle, persistBundle } from "./persist";

const KEY = "mc.bundle.v1";

beforeEach(() => {
  localStorage.clear();
});

test("round-trips a well-formed bundle", () => {
  const b: [string, string][] = [["orders.okf", "# Orders"]];
  persistBundle(b);
  expect(loadPersistedBundle()).toEqual(b);
});

test("returns undefined when nothing is stored", () => {
  expect(loadPersistedBundle()).toBeUndefined();
});

test("returns undefined for invalid JSON", () => {
  localStorage.setItem(KEY, "{not json");
  expect(loadPersistedBundle()).toBeUndefined();
});

test("rejects a tampered bundle whose entries are not [string, string] pairs", () => {
  // A corrupt/tampered value would otherwise reach `build_model`, which throws on
  // a non-`[string,string][]` input, crashing store construction at bootstrap.
  localStorage.setItem(KEY, JSON.stringify([[1, 2]]));
  expect(loadPersistedBundle()).toBeUndefined();
});

test("rejects a bundle that is a non-array JSON value", () => {
  localStorage.setItem(KEY, JSON.stringify({ path: "x" }));
  expect(loadPersistedBundle()).toBeUndefined();
});

test("rejects entries with the wrong arity", () => {
  localStorage.setItem(KEY, JSON.stringify([["a", "b", "c"]]));
  expect(loadPersistedBundle()).toBeUndefined();
  localStorage.setItem(KEY, JSON.stringify([["only-one"]]));
  expect(loadPersistedBundle()).toBeUndefined();
});
