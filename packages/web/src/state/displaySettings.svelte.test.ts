import { test, expect, beforeEach } from "vitest";
import { DEFAULT_DISPLAY } from "@uaml/okf";
import { displaySettings } from "./displaySettings.svelte";

beforeEach(() => displaySettings._reset());

test("resolve returns DEFAULT_DISPLAY for an untouched diagram", () => {
  expect(displaySettings.resolve("d1")).toEqual(DEFAULT_DISPLAY);
});

test("patch overrides a single field, leaving the rest at defaults", () => {
  displaySettings.patch("d1", { showAttributes: false });
  expect(displaySettings.resolve("d1")).toEqual({ ...DEFAULT_DISPLAY, showAttributes: false });
});

test("overrides are isolated per diagram key", () => {
  displaySettings.patch("d1", { associationLabels: "hidden" });
  expect(displaySettings.resolve("d2")).toEqual(DEFAULT_DISPLAY);
});

test("successive patches for the same diagram merge", () => {
  displaySettings.patch("d1", { showAttributes: false });
  displaySettings.patch("d1", { attributeDetail: "name-only" });
  expect(displaySettings.resolve("d1")).toEqual({
    ...DEFAULT_DISPLAY, showAttributes: false, attributeDetail: "name-only",
  });
});

test("a base display is overlaid under the session override", () => {
  const base = { showStereotype: false } as const;
  expect(displaySettings.resolve("d1", base)).toEqual({ ...DEFAULT_DISPLAY, showStereotype: false });
  displaySettings.patch("d1", { showStereotype: true });
  expect(displaySettings.resolve("d1", base).showStereotype).toBe(true);
});
