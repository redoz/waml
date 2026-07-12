import { describe, it, expect, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import Canvas from "./Canvas.svelte";
import { store } from "../../state/model.svelte";

// Reset the shared store singleton between tests so an added node from one test
// doesn't leak into the next.
afterEach(() => {
  store.load([]);
  localStorage.clear();
});

// Dismiss the first-visit WelcomeDialog if present, then drop + select a node via
// the canvas double-click handler (our own DOM handler, deterministic in jsdom).
async function addAndSelectNode() {
  const blank = screen.queryByRole("button", { name: /start blank/i });
  if (blank) await fireEvent.click(blank);
  const wrapper = document.querySelector("[data-canvas-wrapper]") as HTMLElement;
  expect(wrapper).toBeTruthy();
  await fireEvent.dblClick(wrapper);
  await tick();
}

describe("multi-select toolbar + regression", () => {
  it("selecting a node no longer auto-opens the Inspector", async () => {
    render(Canvas);
    await addAndSelectNode();
    // Regression: selection must NOT open the Inspector panel any more.
    expect(screen.queryByRole("complementary", { name: "Inspect" })).toBeNull();
  });

  it("shows the selection toolbar on a non-empty selection", async () => {
    render(Canvas);
    await addAndSelectNode();
    expect(screen.getByTestId("selection-toolbar")).toBeTruthy();
    expect(screen.getByRole("button", { name: /new diagram from selection/i })).toBeTruthy();
  });

  // NOTE: diagram editing (create/rename/membership) is derived-only in Stage 1b —
  // the store's diagram mutators are no-ops (no diagram/membership ops), so the
  // "New diagram from selection" persistence test returns in Stage 1c.
});
