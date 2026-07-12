import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import CentralEditPanel from "./CentralEditPanel.svelte";

// A minimal body snippet containing a focusable text input, so the two-stage
// Esc behaviour can be exercised.
const bodySnippet = createRawSnippet(() => ({
  render: () => `<input aria-label="field" />`,
}));

const props = (over = {}) => ({
  title: "Customer",
  onClose: vi.fn(),
  children: bodySnippet,
  ...over,
});

test("renders the title and body", () => {
  render(CentralEditPanel, { props: props() });
  expect(screen.getByRole("heading", { name: "Customer" })).toBeTruthy();
  expect(screen.getByLabelText("field")).toBeTruthy();
});

test("close button fires onClose", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("clicking the scrim fires onClose", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByTestId("central-scrim"));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Esc with no field focused closes immediately", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Esc while a field is focused blurs first, then a second Esc closes", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  const field = screen.getByLabelText("field") as HTMLInputElement;
  field.focus();
  expect(document.activeElement).toBe(field);

  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).not.toHaveBeenCalled();      // first Esc only blurs
  expect(document.activeElement).not.toBe(field);

  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);     // second Esc closes
});
