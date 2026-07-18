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

  it("opens the read-only switcher dropdown; Edit escalates into the full navigator", async () => {
    render(Canvas);
    await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
    // Read-only dropdown first: the navigator's search field is NOT mounted yet.
    expect(screen.queryByLabelText("Search model")).toBeNull();
    // Edit opens the full editor.
    await fireEvent.click(screen.getByRole("button", { name: /edit model/i }));
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
    // Nothing selected → just the element picker, no hint text.
    expect(within(panel).queryByText(/select an element to edit/i)).toBeNull();
    expect(within(panel).getByRole("combobox", { name: "Select element" })).toBeTruthy();
  });

  it("exposes a pin control that makes the panel translucent while idle", async () => {
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" }) as HTMLElement;
    expect(panel.style.opacity).toBe("1");
    await fireEvent.click(within(panel).getByRole("button", { name: /let it dim when idle/i }));
    await tick();
    expect(panel.style.opacity).toBe("0.4");
    await fireEvent.pointerEnter(panel);
    expect(panel.style.opacity).toBe("1");
  });

  // Task 2 seam: the picker lists the active diagram's member nodes, and
  // choosing one round-trips into canvas selection → the Inspector body.
  it("picker lists active-diagram member nodes; selecting one reflects into the Inspector", async () => {
    const node = store.addNode({ x: 0, y: 0 });
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    const combobox = within(panel).getByRole("combobox", { name: "Select element" });

    // The freshly-added node is a member of the implicit "All" diagram, so it
    // shows up as an option (labelled with its title) once the picker opens.
    await fireEvent.click(combobox);
    await tick();
    const option = within(panel).getByRole("option", { name: node.concept.title! });
    expect(option).toBeTruthy();

    await fireEvent.click(option);
    await tick();

    // Selection round-tripped: the trigger reflects the chosen node and the
    // read-only Inspector body now shows the title as static text (no editable
    // Title input in the docked panel).
    expect(combobox.textContent).toContain(node.concept.title!);
    expect(within(panel).queryByLabelText("Title")).toBeNull();
    expect(within(panel).getAllByText(node.concept.title!).length).toBeGreaterThan(0);
  });

  it("the docked panel's Edit button opens the edit dialog for the selected node", async () => {
    const node = store.addNode({ x: 0, y: 0 });
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    const combobox = within(panel).getByRole("combobox", { name: "Select element" });
    await fireEvent.click(combobox);
    await tick();
    // The store persists across tests, so several same-titled options may exist;
    // the just-added node is the last member, so pick the last matching option.
    const opts = within(panel).getAllByRole("option", { name: node.concept.title! });
    await fireEvent.click(opts[opts.length - 1]);
    await tick();

    await fireEvent.click(within(panel).getByRole("button", { name: "Edit element" }));
    await tick();

    // The centered dialog is now open with the EDITABLE ObjectInspector body.
    const dialog = screen.getByRole("dialog");
    expect(dialog).toBeTruthy();
    expect(within(dialog).getByLabelText("Title")).toBeTruthy();
  });
});

test("slides the tool Dock clear of the docked navigator rail", async () => {
  const { container } = render(Canvas);
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("14px");
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /dock model editor/i }));
  await tick();
  // navWidth default (340) + 12px gap.
  expect(dock.style.left).toBe("352px");
});
