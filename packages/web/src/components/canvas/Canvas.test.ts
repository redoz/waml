import { test, expect, describe, it } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/svelte";
import { tick } from "svelte";
import Canvas from "./Canvas.svelte";

// End-to-end chrome mount check: rendering the provider-wrapped Canvas brings up
// the TopBar, and clicking the first-class top-bar Share button opens the modal
// Share dialog (Share no longer lives in the right rail).
test("mounts the TopBar; clicking top-bar Share opens the Share dialog", async () => {
  render(Canvas);
  expect(screen.getByRole("button", { name: /Templates/ })).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /^Share$/ }));
  expect(screen.getByLabelText("Share URL")).toBeTruthy();
});

describe("diagram title switcher (replaces the goal button + DiagramTabs pill)", () => {
  it("renders the diagram title switcher and no longer renders the Business Goal button", () => {
    render(Canvas);
    // The centered title switcher shows the implicit diagram's default label.
    const switcher = screen.getByRole("button", { name: /switch diagram/i });
    expect(switcher.textContent).toContain("All");
    // The Business Goal button is gone.
    expect(screen.queryByRole("button", { name: "Business goal" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Set business goal" })).toBeNull();
  });

  // NOTE: diagram creation is derived-only in Stage 1b — the store's diagram
  // mutators are no-ops (no diagram/membership ops), so the switcher stays on the
  // implicit "All" view. Persisted diagram creation returns in Stage 1c.
  it("exposes the New diagram affordance in the switcher menu", async () => {
    render(Canvas);
    await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
    expect(screen.getByRole("button", { name: /New diagram/i })).toBeTruthy();
  });
});

describe("right-edge flags", () => {
  it("renders a Feedback flag linking to the GitHub new-issue page in a new tab", () => {
    render(Canvas);
    const feedback = screen.getByRole("link", { name: "Feedback" });
    expect(feedback.getAttribute("href")).toBe("https://github.com/redoz/uaml/issues/new");
    expect(feedback.getAttribute("target")).toBe("_blank");
    expect(feedback.getAttribute("rel") ?? "").toContain("noreferrer");
  });

  it("renders an Inspect flag as a toggle button (the right icon rail is gone)", () => {
    render(Canvas);
    // Exactly one Inspect control now — the right icon rail (which also rendered
    // an 'Inspect' button) has been removed.
    const inspectButtons = screen.getAllByRole("button", { name: "Inspect" });
    expect(inspectButtons).toHaveLength(1);
    // It's a flag toggle: exposes aria-pressed (the old rail button used
    // aria-current instead), and starts unpressed.
    expect(inspectButtons[0].getAttribute("aria-pressed")).toBe("false");
  });

  it("no longer renders the bottom-left Google Form feedback anchor", () => {
    render(Canvas);
    const links = screen.getAllByRole("link");
    expect(links.some((a) => (a.getAttribute("href") ?? "").includes("forms.gle"))).toBe(false);
  });
});

describe("Inspect flag + pinnable Inspector", () => {
  it("toggles the Inspector open and closed", async () => {
    render(Canvas);
    // Closed initially (nothing selected).
    expect(screen.queryByRole("complementary", { name: "Inspect" })).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    const panel = screen.getByRole("complementary", { name: "Inspect" });
    expect(panel).toBeTruthy();
    // Hosted in the dedicated pinnable InspectorPanel (has a pin control).
    expect(within(panel).getByRole("button", { name: /pin/i })).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    expect(screen.queryByRole("complementary", { name: "Inspect" })).toBeNull();
  });

  it("pinning keeps the Inspector open and translucent while idle", async () => {
    render(Canvas);
    await fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    const panel = screen.getByRole("complementary", { name: "Inspect" });

    // Unpinned + idle → opaque.
    expect(panel.classList.contains("opacity-40")).toBe(false);

    await fireEvent.click(within(panel).getByRole("button", { name: /pin inspector/i }));
    await tick();
    // Pinned + idle → translucent, and still open.
    expect(panel.classList.contains("opacity-40")).toBe(true);

    // Hover restores opacity.
    await fireEvent.pointerEnter(panel);
    expect(panel.classList.contains("opacity-40")).toBe(false);
  });
});
