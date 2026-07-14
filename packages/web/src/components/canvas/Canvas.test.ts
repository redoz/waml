import { test, expect, describe, it } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/svelte";
import { tick } from "svelte";
import Canvas from "./Canvas.svelte";
import { store } from "../../state/model.svelte";

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

  // The switcher trigger now opens the Navigator sheet (search / scope / create /
  // rename / reorder / delete) wired to the live model store. Its search field is
  // the tell that the sheet mounted in place of the old inline diagram list.
  it("opens the Navigator sheet from the switcher", async () => {
    render(Canvas);
    await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
    expect(screen.getByLabelText("Search model")).toBeTruthy();
  });
});

describe("right-edge flags", () => {
  it("renders a Feedback flag linking to the GitHub new-issue page in a new tab", () => {
    render(Canvas);
    const feedback = screen.getByRole("link", { name: "Feedback" });
    expect(feedback.getAttribute("href")).toBe("https://github.com/redoz/waml/issues/new");
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

  // Task 2 seam: the picker lists the active diagram's member nodes, and
  // choosing one round-trips into canvas selection → the Inspector body.
  it("picker lists active-diagram member nodes; selecting one reflects into the Inspector", async () => {
    const node = store.addNode({ x: 0, y: 0 });
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    const combobox = within(panel).getByRole("combobox", { name: "Select element" }) as HTMLSelectElement;

    // The freshly-added node is a member of the implicit "All" diagram, so it
    // shows up as an option labelled with its title.
    expect(within(panel).getByRole("option", { name: node.concept.title! })).toBeTruthy();

    await fireEvent.change(combobox, { target: { value: node.key } });
    await tick();

    // Selection round-tripped: the combobox reflects the chosen node, the hint
    // is gone, and the Inspector body now shows that node's title field.
    expect(combobox.value).toBe(node.key);
    expect(within(panel).queryByText(/select an element to edit/i)).toBeNull();
    expect((within(panel).getByLabelText("Title") as HTMLInputElement).value).toBe(node.concept.title);
  });
});
