import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { TEMPLATES } from "@waml/core/templates";
import LibraryDialog from "./LibraryDialog.svelte";

test("Use rolls out the first template", async () => {
  const onUse = vi.fn();
  render(LibraryDialog, { props: { onUse, onClose: vi.fn() } });
  const first = TEMPLATES[0];
  const useButtons = screen.getAllByRole("button", { name: /Use/ });
  await fireEvent.click(useButtons[0]);
  // onUse now receives the template's `.okf` bundle (`[path, markdown][]`).
  const [bundle, name] = onUse.mock.calls[0];
  expect(Array.isArray(bundle)).toBe(true);
  expect(bundle[0]).toHaveLength(2);
  expect(typeof bundle[0][0]).toBe("string");
  expect(name).toBe(first.name);
});

test("lists all four templates", () => {
  render(LibraryDialog, { props: { onUse: vi.fn(), onClose: vi.fn() } });
  // Exact match: one template is named "Orders Use Cases", whose row header
  // is itself an accessible `role="button"` with that name, so the loose
  // /Use/ substring regex used above also (correctly, for that test's needs)
  // matches it — inflating the count here. Exact "Use" isolates the Rocket
  // "Use" buttons only.
  const useButtons = screen.getAllByRole("button", { name: "Use" });
  expect(TEMPLATES).toHaveLength(4);
  expect(useButtons).toHaveLength(TEMPLATES.length);
});
