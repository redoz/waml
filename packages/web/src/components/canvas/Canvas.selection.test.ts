import { describe, it, expect, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import Canvas from "./Canvas.svelte";
import { store } from "../../state/model.svelte";

// Reset the shared store singleton between tests so an added node from one test
// doesn't leak into the next.
afterEach(() => {
  store.set({ nodes: [], edges: [], diagrams: [] });
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

  it("'New diagram from selection' creates a diagram with exactly the selected node and activates it", async () => {
    render(Canvas);
    await addAndSelectNode();
    const nodeKey = store.get().nodes[0].key;

    await fireEvent.click(screen.getByRole("button", { name: /new diagram from selection/i }));
    const input = screen.getByLabelText("New diagram name") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Focus" } });
    await fireEvent.click(screen.getByRole("button", { name: /^create diagram$/i }));
    await tick();

    // Store: a new diagram seeded with exactly the selected node.
    const diagrams = store.get().diagrams;
    expect(diagrams).toHaveLength(1);
    expect(diagrams[0].title).toBe("Focus");
    expect(diagrams[0].members).toEqual([nodeKey]);

    // Activated: the TopBar switcher now reflects the new diagram.
    expect(screen.getByRole("button", { name: /Diagram: Focus/i })).toBeTruthy();
  });
});
