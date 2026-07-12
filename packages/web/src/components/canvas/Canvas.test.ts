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

  it("no longer renders the bottom-left Google Form feedback anchor", () => {
    render(Canvas);
    const links = screen.getAllByRole("link");
    expect(links.some((a) => (a.getAttribute("href") ?? "").includes("forms.gle"))).toBe(false);
  });
});

describe("pinnable Inspector (always present, never closes)", () => {
  it("is always mounted, even with nothing selected", () => {
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    expect(panel).toBeTruthy();
    // Nothing selected → shows the hint and the element picker.
    expect(within(panel).getByText(/select an element to edit/i)).toBeTruthy();
    expect(within(panel).getByRole("combobox", { name: "Select element" })).toBeTruthy();
  });

  it("exposes a pin control that makes the panel translucent while idle", async () => {
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    expect(panel.classList.contains("opacity-40")).toBe(false);
    await fireEvent.click(within(panel).getByRole("button", { name: /pin inspector/i }));
    await tick();
    expect(panel.classList.contains("opacity-40")).toBe(true);
    await fireEvent.pointerEnter(panel);
    expect(panel.classList.contains("opacity-40")).toBe(false);
  });
});
