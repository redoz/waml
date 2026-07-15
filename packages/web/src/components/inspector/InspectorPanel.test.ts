import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import InspectorPanel from "./InspectorPanel.svelte";

type Kind = "diagram" | "node" | "edge";
const OPTIONS: { key: string; label: string; kind: Kind }[] = [
  { key: "d1", label: "My Diagram", kind: "diagram" },
  { key: "customer", label: "Customer", kind: "node" },
  { key: "order", label: "Order", kind: "node" },
  { key: "e1", label: "Customer → Order", kind: "edge" },
];

function setup(props: Record<string, unknown> = {}) {
  return render(InspectorPanel, {
    props: {
      options: OPTIONS,
      selectedKey: "customer",
      focusedKind: "node",
      onSelect: vi.fn(),
      pinned: false,
      onTogglePin: vi.fn(),
      ...props,
    },
  });
}

describe("InspectorPanel", () => {
  it("always renders the region (never closes)", () => {
    setup({ selectedKey: null, focusedKind: undefined });
    expect(screen.getByRole("complementary", { name: "Inspector" })).toBeTruthy();
  });

  it("the trigger reflects the selected element's label", () => {
    setup({ selectedKey: "order" });
    const trigger = screen.getByRole("combobox", { name: "Select element" });
    expect(trigger.textContent).toContain("Order");
  });

  it("shows placeholder text on the trigger when nothing is selected", () => {
    setup({ selectedKey: null, focusedKind: undefined });
    const trigger = screen.getByRole("combobox", { name: "Select element" });
    expect(trigger.textContent).toMatch(/select an element/i);
  });

  it("opens a listbox of the diagram, its objects and associations", async () => {
    setup({ selectedKey: "order" });
    const trigger = screen.getByRole("combobox", { name: "Select element" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    await fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByRole("option", { name: "My Diagram" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Customer" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Customer → Order" })).toBeTruthy();
    const selected = screen.getByRole("option", { name: "Order" });
    expect(selected.getAttribute("aria-selected")).toBe("true");
  });

  it("fires onSelect with the chosen key and kind, and closes the listbox", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    await fireEvent.click(screen.getByRole("combobox", { name: "Select element" }));
    await fireEvent.click(screen.getByRole("option", { name: "Order" }));
    expect(onSelect).toHaveBeenCalledWith("order", "node");
    expect(
      screen.getByRole("combobox", { name: "Select element" }).getAttribute("aria-expanded"),
    ).toBe("false");
  });

  it("fires onSelect with the edge kind for an association row", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    await fireEvent.click(screen.getByRole("combobox", { name: "Select element" }));
    await fireEvent.click(screen.getByRole("option", { name: "Customer → Order" }));
    expect(onSelect).toHaveBeenCalledWith("e1", "edge");
  });

  it("fires onSelect with the diagram kind for the diagram row", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    await fireEvent.click(screen.getByRole("combobox", { name: "Select element" }));
    await fireEvent.click(screen.getByRole("option", { name: "My Diagram" }));
    expect(onSelect).toHaveBeenCalledWith("d1", "diagram");
  });

  it("closes the listbox on Escape", async () => {
    setup();
    const trigger = screen.getByRole("combobox", { name: "Select element" });
    await fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    await fireEvent.keyDown(trigger, { key: "Escape" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
  });

  it("with nothing focused: no hint text, no collapse control, no kind icon", () => {
    const { container } = setup({ selectedKey: null, focusedKind: undefined });
    expect(screen.queryByText(/select an element to edit/i)).toBeNull();
    expect(screen.queryByRole("button", { name: /collapse inspector/i })).toBeNull();
    expect(container.querySelector(".inspector-kind")).toBeNull();
  });

  it("with a node focused: shows the kind icon and a collapse control", () => {
    const { container } = setup({ focusedKind: "node" });
    expect(container.querySelector(".inspector-kind svg")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Collapse inspector" })).toBeTruthy();
  });

  it("with the diagram focused: shows a kind icon and an Edit button that fires onEdit", async () => {
    const onEdit = vi.fn();
    const { container } = setup({ selectedKey: "d1", focusedKind: "diagram", onEdit });
    expect(container.querySelector(".inspector-kind svg")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Edit element" }));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it("collapse toggle flips aria-expanded and its label", async () => {
    setup({ focusedKind: "node" });
    const collapse = screen.getByRole("button", { name: "Collapse inspector" });
    expect(collapse.getAttribute("aria-expanded")).toBe("true");
    await fireEvent.click(collapse);
    const expand = screen.getByRole("button", { name: "Expand inspector" });
    expect(expand.getAttribute("aria-expanded")).toBe("false");
  });

  it("fires onTogglePin when the pin control is clicked", async () => {
    const onTogglePin = vi.fn();
    setup({ onTogglePin });
    await fireEvent.click(screen.getByRole("button", { name: /pin inspector/i }));
    expect(onTogglePin).toHaveBeenCalledTimes(1);
  });

  it("is opaque when unpinned and translucent when pinned + idle", () => {
    setup({ pinned: false });
    expect(screen.getByRole("complementary").classList.contains("opacity-40")).toBe(false);
    setup({ pinned: true });
    const asides = screen.getAllByRole("complementary");
    expect(asides[asides.length - 1].classList.contains("opacity-40")).toBe(true);
  });

  it("becomes opaque on hover, translucent again after the pointer leaves", async () => {
    setup({ pinned: true, hideDelay: 20 });
    const aside = screen.getByRole("complementary");
    expect(aside.classList.contains("opacity-40")).toBe(true);
    await fireEvent.pointerEnter(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);
    await fireEvent.pointerLeave(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);
    await new Promise((r) => setTimeout(r, 40));
    await tick();
    expect(aside.classList.contains("opacity-40")).toBe(true);
  });

  it("shows an Edit button only when a node/edge is focused, and fires onEdit", async () => {
    const onEdit = vi.fn();
    const { unmount } = setup({ selectedKey: null, focusedKind: undefined, onEdit });
    expect(screen.queryByRole("button", { name: "Edit element" })).toBeNull();
    unmount();
    setup({ focusedKind: "node", onEdit });
    await fireEvent.click(screen.getByRole("button", { name: "Edit element" }));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });
});
