import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { INDUSTRY_TEMPLATES } from "@uaml/core/templates";
import LibraryDialog from "./LibraryDialog.svelte";

test("Use rolls out the first template", async () => {
  const onUse = vi.fn();
  render(LibraryDialog, { props: { onUse, onClose: vi.fn() } });
  const first = INDUSTRY_TEMPLATES[0];
  const useButtons = screen.getAllByRole("button", { name: /Use/ });
  await fireEvent.click(useButtons[0]);
  expect(onUse).toHaveBeenCalledWith(expect.objectContaining({ nodes: expect.any(Array) }), first.name);
});
