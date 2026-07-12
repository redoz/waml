import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import InspectorPanel from "./InspectorPanel.svelte";

const OPTIONS = [
  { key: "customer", label: "Customer" },
  { key: "order", label: "Order" },
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

  it("renders the picker options and reflects the selected key", () => {
    setup({ selectedKey: "order" });
    const select = screen.getByRole("combobox", { name: "Select element" }) as HTMLSelectElement;
    expect(select.value).toBe("order");
    expect(screen.getByRole("option", { name: "Customer" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Order" })).toBeTruthy();
  });

  it("fires onSelect with the chosen key", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    const select = screen.getByRole("combobox", { name: "Select element" });
    await fireEvent.change(select, { target: { value: "order" } });
    expect(onSelect).toHaveBeenCalledWith("order");
  });

  it("fires onSelect(null) when the placeholder is chosen", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    const select = screen.getByRole("combobox", { name: "Select element" });
    await fireEvent.change(select, { target: { value: "" } });
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("with nothing focused: shows a hint, no collapse control, no kind icon", () => {
    const { container } = setup({ selectedKey: null, focusedKind: undefined });
    expect(screen.getByText(/select an element to edit/i)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /collapse inspector/i })).toBeNull();
    expect(container.querySelector(".inspector-kind")).toBeNull();
  });

  it("with a node focused: shows the kind icon and a collapse control, no hint", () => {
    const { container } = setup({ focusedKind: "node" });
    expect(container.querySelector(".inspector-kind svg")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Collapse inspector" })).toBeTruthy();
    expect(screen.queryByText(/select an element to edit/i)).toBeNull();
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
});
