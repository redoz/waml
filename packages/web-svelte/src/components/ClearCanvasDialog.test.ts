import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ClearCanvasDialog from "./ClearCanvasDialog.svelte";

test("Delete / Export-&-delete route to their handlers", async () => {
  const onDelete = vi.fn();
  const onExportAndDelete = vi.fn();
  render(ClearCanvasDialog, {
    props: { counts: { marts: 3, relationships: 2 }, onDelete, onExportAndDelete, onClose: vi.fn() },
  });
  expect(document.body.textContent).toContain("3 marts");
  await fireEvent.click(screen.getByRole("button", { name: /Export OKF & delete/ }));
  expect(onExportAndDelete).toHaveBeenCalledTimes(1);
  await fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  expect(onDelete).toHaveBeenCalledTimes(1);
});
