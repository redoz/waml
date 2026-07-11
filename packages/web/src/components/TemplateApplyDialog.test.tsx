import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { ModelGraph } from "@mc/okf";
import { TemplateApplyDialog } from "./TemplateApplyDialog";

const graph: ModelGraph = {
  nodes: [
    { key: "a", title: "Orders", type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
    { key: "b", title: "Customers", type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
  ],
  edges: [{ id: "e1", kind: "associates", from: "a", to: "b", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false }],
  diagrams: [],
};

describe("TemplateApplyDialog", () => {
  it("shows the template name and the mart/relationship counts", () => {
    render(<TemplateApplyDialog graph={graph} name="E-commerce" onConfirm={() => {}} onClose={() => {}} />);
    expect(screen.getByText(/E-commerce/)).toBeTruthy();
    expect(screen.getByText(/Will import 2 marts, 1 relationships\./)).toBeTruthy();
  });

  it("confirms with the default replace mode", () => {
    const onConfirm = vi.fn();
    render(<TemplateApplyDialog graph={graph} name="E-commerce" onConfirm={onConfirm} onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /apply/i }));
    expect(onConfirm).toHaveBeenCalledWith("replace");
  });

  it("confirms with merge when selected", () => {
    const onConfirm = vi.fn();
    render(<TemplateApplyDialog graph={graph} name="E-commerce" onConfirm={onConfirm} onClose={() => {}} />);
    fireEvent.click(screen.getByText("Merge into the canvas"));
    fireEvent.click(screen.getByRole("button", { name: /apply/i }));
    expect(onConfirm).toHaveBeenCalledWith("merge");
  });
});
