import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ExternalRefs } from "./ExternalRefs";
import type { ModelEdge, ModelNode, Diagram } from "@mc/okf";

const node = (key: string, title: string): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const nodes = [node("order", "Order"), node("money", "Money"), node("checkout", "Checkout")];
const edges: ModelEdge[] = [
  { id: "e1", kind: "associates", from: "order", to: "money", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false },
  { id: "e2", kind: "depends", from: "checkout", to: "order", fromEnd: {}, toEnd: {}, bidirectional: false },
  { id: "e3", kind: "associates", from: "order", to: "order2x", fromEnd: {}, toEnd: {}, bidirectional: false }, // dangling → ignored
];
const diagrams: Diagram[] = [
  { key: "domain", title: "Domain", profile: "uml-domain", members: ["order"] },
  { key: "shared", title: "Shared", profile: "uml-domain", members: ["money", "checkout"] },
];

describe("ExternalRefs", () => {
  it("lists incoming and outgoing off-diagram relationships as chips", () => {
    render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges} members={["order"]} diagrams={diagrams} onNavigate={() => {}} />);
    expect(screen.getByText(/associates → Money/)).toBeTruthy();
    expect(screen.getByText(/Checkout → depends/)).toBeTruthy();
  });
  it("clicking a chip navigates to a diagram containing the other node", () => {
    const onNavigate = vi.fn();
    render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges} members={["order"]} diagrams={diagrams} onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText(/associates → Money/));
    expect(onNavigate).toHaveBeenCalledWith("shared", "money");
  });
  it("renders nothing when every relationship is on-diagram", () => {
    const { container } = render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges}
      members={["order", "money", "checkout"]} diagrams={diagrams} onNavigate={() => {}} />);
    expect(container.firstChild).toBeNull();
  });
});
