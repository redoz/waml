import { test, expect, vi } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/svelte";
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

// Guards the exact confirmation copy — Svelte whitespace-collapse around the
// {#if} block must not inject a stray space before the period (a fidelity bug
// vs ClearCanvasDialog.tsx, worse in the empty case which double-spaced).
test("confirmation copy has no stray whitespace (non-empty)", () => {
  render(ClearCanvasDialog, {
    props: { counts: { marts: 3, relationships: 2 }, onDelete: vi.fn(), onExportAndDelete: vi.fn(), onClose: vi.fn() },
  });
  expect(document.body.textContent).toContain(
    "This permanently deletes everything on the canvas — 3 marts and 2 relationships. This can't be undone.",
  );
  cleanup();
});

test("confirmation copy has no stray whitespace (empty canvas)", () => {
  render(ClearCanvasDialog, {
    props: { counts: { marts: 0, relationships: 0 }, onDelete: vi.fn(), onExportAndDelete: vi.fn(), onClose: vi.fn() },
  });
  expect(document.body.textContent).toContain(
    "This permanently deletes everything on the canvas. This can't be undone.",
  );
});
