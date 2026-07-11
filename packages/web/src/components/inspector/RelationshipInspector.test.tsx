import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RelationshipInspector } from "./RelationshipInspector";
import type { ModelEdge, ModelNode } from "@mc/okf";

const node = (key: string, title: string): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge: ModelEdge = { id: "e1", kind: "associates", from: "a", to: "b",
  fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*", navigable: true }, bidirectional: false };

describe("RelationshipInspector", () => {
  it("changes the kind through the verb select", () => {
    const onUpdate = vi.fn();
    render(<RelationshipInspector edge={edge} fromNode={node("a", "Order")} toNode={node("b", "OrderLine")} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "composes" } });
    expect(onUpdate).toHaveBeenCalledWith({ kind: "composes" });
  });
  it("edits the near-end multiplicity", () => {
    const onUpdate = vi.fn();
    render(<RelationshipInspector edge={edge} fromNode={node("a", "Order")} toNode={node("b", "OrderLine")} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByLabelText("Order multiplicity"), { target: { value: "0..1" } });
    expect(onUpdate).toHaveBeenCalledWith({ fromEnd: { multiplicity: "0..1" } });
  });
  it("hides end editors for specializes", () => {
    render(<RelationshipInspector edge={{ ...edge, kind: "specializes", fromEnd: {}, toEnd: {} }} fromNode={node("a", "A")} toNode={node("b", "B")} onUpdate={() => {}} />);
    expect(screen.queryByLabelText("A multiplicity")).toBeNull();
  });
});
