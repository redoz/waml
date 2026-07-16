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
// Fires a pointerEnter first — realistically the pointer is over the canvas
// wrapper when double-clicking it, and the SelectionToolbar now also requires
// hover (in addition to a non-empty selection) to show.
async function addAndSelectNode() {
  const blank = screen.queryByRole("button", { name: /start blank/i });
  if (blank) await fireEvent.click(blank);
  const wrapper = document.querySelector("[data-canvas-wrapper]") as HTMLElement;
  expect(wrapper).toBeTruthy();
  await fireEvent.pointerEnter(wrapper);
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

  it("hides the toolbar once the pointer leaves the canvas, and re-shows it on re-hover", async () => {
    render(Canvas);
    await addAndSelectNode();
    expect(screen.getByTestId("selection-toolbar")).toBeTruthy();

    const wrapper = document.querySelector("[data-canvas-wrapper]") as HTMLElement;
    await fireEvent.pointerLeave(wrapper);
    await tick();
    // Selection is still non-empty, but without hover the toolbar must hide.
    expect(screen.queryByTestId("selection-toolbar")).toBeNull();

    await fireEvent.pointerEnter(wrapper);
    await tick();
    expect(screen.getByTestId("selection-toolbar")).toBeTruthy();
  });
});

// Final-whole-branch-review fix: a selection made in one diagram must not
// survive a switch to another diagram — most importantly, it must never carry
// into a read-only Flow/Sequence view, where a stale selection would leave the
// floating SelectionToolbar's Delete button live against the (still-mounted)
// model. Exercised here with two ordinary diagrams — the switcher's
// selection-clearing fix is generic to every diagram switch, not special-cased
// to behavior views.
describe("diagram switch clears a stale selection (final-review fix)", () => {
  it("clears the selection when the switcher activates a different diagram", async () => {
    store.load([
      ["alpha.md", "---\ntype: uml.Class\ntitle: Alpha\n---\n# Alpha\n"],
      ["beta.md", "---\ntype: uml.Class\ntitle: Beta\n---\n# Beta\n"],
      [
        "one.md",
        "---\ntype: Diagram\ntitle: One\nprofile: uml-domain\n---\n# One\n\n## Members\n\n### Items\n- [Alpha](./alpha.md)\n",
      ],
      [
        "two.md",
        "---\ntype: Diagram\ntitle: Two\nprofile: uml-domain\n---\n# Two\n\n## Members\n\n### Items\n- [Beta](./beta.md)\n",
      ],
    ]);
    render(Canvas);

    // Select an element in whichever diagram is active on mount.
    await addAndSelectNode();
    expect(screen.getByTestId("selection-toolbar")).toBeTruthy();

    // Open the switcher and activate the OTHER diagram (whichever one isn't
    // currently shown as the trigger's title).
    const switcher = screen.getByRole("button", { name: /switch diagram/i });
    const otherTitle = switcher.textContent?.includes("One") ? "Two" : "One";
    await fireEvent.click(switcher);
    await fireEvent.click(screen.getByRole("option", { name: otherTitle }));
    await tick();

    // The stale selection from the previous diagram must be gone — no toolbar,
    // and the switcher now shows the newly-activated diagram.
    expect(screen.queryByTestId("selection-toolbar")).toBeNull();
    expect(screen.getByRole("button", { name: /switch diagram/i }).textContent).toContain(otherTitle);
  });
});
