import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { DATASET_TEMPLATES } from "@uaml/core/templates";
import LibraryDialog from "./LibraryDialog.svelte";

test("Use rolls out the first template", async () => {
  const onUse = vi.fn();
  render(LibraryDialog, { props: { onUse, onClose: vi.fn() } });
  const first = DATASET_TEMPLATES[0];
  const useButtons = screen.getAllByRole("button", { name: /Use/ });
  await fireEvent.click(useButtons[0]);
  // onUse now receives the template's `.okf` bundle (`[path, markdown][]`).
  const [bundle, name] = onUse.mock.calls[0];
  expect(Array.isArray(bundle)).toBe(true);
  expect(bundle[0]).toHaveLength(2);
  expect(typeof bundle[0][0]).toBe("string");
  expect(name).toBe(first.name);
});
