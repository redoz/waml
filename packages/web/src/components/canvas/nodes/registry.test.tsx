import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { resolveNodeRenderer } from "./registry";
import { GenericNode } from "./GenericNode";
import type { ModelNode } from "@mc/okf";

const node = (over: Partial<ModelNode>): ModelNode =>
  ({ key: "n1", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 }, ...over });

const draw = (n: ModelNode) => {
  const C = resolveNodeRenderer(n.type);
  render(<ReactFlowProvider><C data={{ ...n, _viewMode: "erd" }} /></ReactFlowProvider>);
};

describe("metaclass renderer registry", () => {
  it("unknown family falls back to the generic box (never errors)", () => {
    expect(resolveNodeRenderer("bpmn.Task")).toBe(GenericNode);
    expect(resolveNodeRenderer("Data Mart")).toBe(GenericNode);
    expect(resolveNodeRenderer("uml.Nope")).toBe(GenericNode);
  });
  it("uml.Association resolves to a dedicated renderer (class box with attributes)", () => {
    expect(resolveNodeRenderer("uml.Association")).not.toBe(GenericNode);
    draw(node({ type: "uml.Association", title: "Places",
      attributes: [{ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" }] }));
    expect(screen.getByText("Places")).toBeTruthy();
    expect(screen.getByText("placedAt")).toBeTruthy();
  });
  it("uml.Note renders its body in a dog-eared box with no attribute compartment", () => {
    expect(resolveNodeRenderer("uml.Note")).not.toBe(GenericNode);
    draw(node({ type: "uml.Note", title: "Domestic-only", body: "Only for domestic customers.",
      attributes: [{ name: "shouldNotRender", type: { name: "X" }, multiplicity: "1" }] }));
    expect(screen.getByText("Only for domestic customers.")).toBeTruthy();
    expect(screen.queryByText("shouldNotRender")).toBeNull();
  });
  it("uml.Class renders stereotypes in guillemets and italic abstract name", () => {
    draw(node({ stereotypes: ["aggregateRoot"], abstract: true,
      attributes: [{ name: "id", type: { name: "OrderId" }, multiplicity: "1" }] }));
    expect(screen.getByText("«aggregateRoot»")).toBeTruthy();
    const title = screen.getByText("Order");
    expect(title.className).toContain("italic");
    expect(screen.getByText("id")).toBeTruthy();
  });
  it("uml.Interface shows the «interface» keyword", () => {
    draw(node({ type: "uml.Interface", title: "PricingService" }));
    expect(screen.getByText("«interface»")).toBeTruthy();
  });
  it("uml.Enum lists its literals under «enumeration»", () => {
    draw(node({ type: "uml.Enum", title: "OrderStatus", values: ["DRAFT", "PLACED"] }));
    expect(screen.getByText("«enumeration»")).toBeTruthy();
    expect(screen.getByText("DRAFT")).toBeTruthy();
  });
  it("generic box still shows title and attributes", () => {
    draw(node({ type: "whatever", attributes: [{ name: "x", type: { name: "Y" }, multiplicity: "1" }] }));
    expect(screen.getByText("Order")).toBeTruthy();
    expect(screen.getByText("x")).toBeTruthy();
  });
  it("stereotype styles from the profile decorate the box", () => {
    const C = resolveNodeRenderer("uml.Class");
    const { container } = render(
      <ReactFlowProvider>
        <C data={{ ...node({ stereotypes: ["aggregateRoot"] }), _viewMode: "erd", _profile: "uml-domain" }} />
      </ReactFlowProvider>,
    );
    const box = container.querySelector("[data-stereotyped]") as HTMLElement;
    expect(box.style.borderColor).toBe("rgb(234, 179, 8)");
  });
});
