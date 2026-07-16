import { describe, it, test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import SelectionToolbar from "./SelectionToolbar.svelte";

describe("SelectionToolbar", () => {
  it("renders the two actions when a selection is present", () => {
    render(SelectionToolbar, { nodeCount: 2, edgeCount: 0, onNewDiagram: () => {}, onDelete: () => {} });
    expect(screen.getByRole("button", { name: /new diagram from selection/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /delete selection/i })).toBeTruthy();
  });

  it("docks to the bottom-center of the viewport", () => {
    render(SelectionToolbar, { nodeCount: 1, edgeCount: 0, onNewDiagram: () => {}, onDelete: () => {} });
    const bar = screen.getByTestId("selection-toolbar") as HTMLElement;
    expect(bar.className).toContain("fixed");
    expect(bar.className).toContain("bottom-6");
    expect(bar.className).toContain("left-1/2");
  });

  it("disables 'New diagram from selection' when only edges are selected", () => {
    render(SelectionToolbar, { nodeCount: 0, edgeCount: 2, onNewDiagram: () => {}, onDelete: () => {} });
    const btn = screen.getByRole("button", { name: /new diagram from selection/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it("creates a diagram from the inline name input", async () => {
    const onNewDiagram = vi.fn();
    render(SelectionToolbar, { nodeCount: 1, edgeCount: 0, onNewDiagram, onDelete: () => {} });
    await fireEvent.click(screen.getByRole("button", { name: /new diagram from selection/i }));
    const input = screen.getByLabelText("New diagram name") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Focus view" } });
    await fireEvent.click(screen.getByRole("button", { name: /^create diagram$/i }));
    expect(onNewDiagram).toHaveBeenCalledWith("Focus view");
  });

  it("rejects an empty / whitespace name (does not call onNewDiagram)", async () => {
    const onNewDiagram = vi.fn();
    render(SelectionToolbar, { nodeCount: 1, edgeCount: 0, onNewDiagram, onDelete: () => {} });
    await fireEvent.click(screen.getByRole("button", { name: /new diagram from selection/i }));
    const input = screen.getByLabelText("New diagram name") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "   " } });
    await fireEvent.click(screen.getByRole("button", { name: /^create diagram$/i }));
    expect(onNewDiagram).not.toHaveBeenCalled();
  });

  it("calls onDelete when 'Delete selection' is clicked", async () => {
    const onDelete = vi.fn();
    render(SelectionToolbar, { nodeCount: 1, edgeCount: 1, onNewDiagram: () => {}, onDelete });
    await fireEvent.click(screen.getByRole("button", { name: /delete selection/i }));
    expect(onDelete).toHaveBeenCalledOnce();
  });
});

test("the Delete button shows the ⌫ key hint", () => {
  render(SelectionToolbar, {
    props: {
      nodeCount: 1,
      edgeCount: 0,
      onNewDiagram: vi.fn(),
      onDelete: vi.fn(),
    },
  });
  const del = screen.getByRole("button", { name: "Delete selection" });
  const kbd = del.querySelector("kbd");
  expect(kbd?.textContent).toBe("⌫"); // ⌫
});
