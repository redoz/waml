import { test, expect } from "vitest";
import { SHORTCUTS, shortcut, keyLabel, matchesShortcut } from "./shortcuts";

test("every shortcut has a unique id, non-empty event + display, and a label", () => {
  const ids = SHORTCUTS.map((s) => s.id);
  expect(new Set(ids).size).toBe(ids.length);
  for (const s of SHORTCUTS) {
    expect(s.event.length).toBeGreaterThan(0);
    expect(s.display.length).toBeGreaterThan(0);
    expect(s.label.length).toBeGreaterThan(0);
  }
});

test("keyLabel returns the display glyphs", () => {
  expect(keyLabel("tool.select")).toEqual(["V"]);
  expect(keyLabel("selection.delete")).toEqual(["⌫"]); // ⌫
  expect(keyLabel("hints.toggle")).toEqual(["?"]);
});

test("shortcut throws on an unknown id", () => {
  // @ts-expect-error unknown id
  expect(() => shortcut("nope")).toThrow(/unknown shortcut/);
});

test("matchesShortcut compares against KeyboardEvent.key", () => {
  expect(matchesShortcut("tool.select", new KeyboardEvent("keydown", { key: "v" }))).toBe(true);
  expect(matchesShortcut("tool.select", new KeyboardEvent("keydown", { key: "x" }))).toBe(false);
  // delete binds both Delete and Backspace
  expect(matchesShortcut("selection.delete", new KeyboardEvent("keydown", { key: "Delete" }))).toBe(true);
  expect(matchesShortcut("selection.delete", new KeyboardEvent("keydown", { key: "Backspace" }))).toBe(true);
  expect(matchesShortcut("hints.toggle", new KeyboardEvent("keydown", { key: "?" }))).toBe(true);
});
