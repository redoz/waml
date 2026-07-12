import { describe, it, expect } from "vitest";
import { toModelGraph, edgeKey, emptyOverlay, type Overlay, type RustModel } from "./overlay";

// A minimal Rust `Model` (as serialized from wasm `build_model`) for adapter tests.
function model(partial: Partial<RustModel>): RustModel {
  return { nodes: [], edges: [], diagrams: [], ...partial };
}

describe("toModelGraph", () => {
  it("flattens a diagram's group forest to flat members in declared, depth-first order", () => {
    const m = model({
      nodes: [
        { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [] },
        { key: "line", type: "uml.Class", title: "OrderLine", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [] },
        { key: "money", type: "uml.DataType", title: "Money", stereotypes: [], attributes: [] },
      ],
      diagrams: [
        {
          key: "d1",
          title: "D1",
          profile: "uml-domain",
          groups: [
            {
              name: "A",
              members: ["order", "customer"],
              children: [{ name: "B", members: ["line"], children: [] }],
            },
            { name: "C", members: ["money"], children: [] },
          ],
        },
      ],
    });
    const g = toModelGraph(m, emptyOverlay());
    expect(g.diagrams).toHaveLength(1);
    // group A members, then A's child B, then group C — declared/depth-first.
    expect(g.diagrams[0].members).toEqual(["order", "customer", "line", "money"]);
    expect(g.diagrams[0].key).toBe("d1");
  });

  it("injects node position from the overlay; missing positions default to {0,0}", () => {
    const m = model({
      nodes: [
        { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [] },
      ],
    });
    const overlay: Overlay = emptyOverlay();
    overlay.nodes.set("order", { position: { x: 40, y: 90 } });
    const g = toModelGraph(m, overlay);
    expect(g.nodes.find((n) => n.key === "order")!.position).toEqual({ x: 40, y: 90 });
    expect(g.nodes.find((n) => n.key === "customer")!.position).toEqual({ x: 0, y: 0 });
  });

  it("carries edge handles and synthetic e# ids from the overlay", () => {
    const m = model({
      nodes: [
        { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [] },
      ],
      edges: [
        {
          kind: "associates",
          from: "order",
          to: "customer",
          fromEnd: { multiplicity: "1" },
          toEnd: { multiplicity: "1" },
          bidirectional: false,
        },
      ],
    });
    const overlay: Overlay = emptyOverlay();
    overlay.edges.set(edgeKey({ from: "order", to: "customer", kind: "associates" }), {
      id: "e7",
      sourceHandle: "right",
      targetHandle: "left",
    });
    const g = toModelGraph(m, overlay);
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].id).toBe("e7");
    expect(g.edges[0].sourceHandle).toBe("right");
    expect(g.edges[0].targetHandle).toBe("left");
    expect(g.edges[0].from).toBe("order");
    expect(g.edges[0].to).toBe("customer");
    expect(g.edges[0].bidirectional).toBe(false);
  });

  it("synthesizes an e# id when the overlay has no entry for an edge", () => {
    const m = model({
      nodes: [
        { key: "a", type: "uml.Class", title: "A", stereotypes: [], attributes: [] },
        { key: "b", type: "uml.Class", title: "B", stereotypes: [], attributes: [] },
      ],
      edges: [{ kind: "depends", from: "a", to: "b", fromEnd: {}, toEnd: {}, bidirectional: false }],
    });
    const g = toModelGraph(m, emptyOverlay());
    expect(g.edges[0].id).toBe("e1");
  });

  it("empty diagrams yields a ModelGraph with diagrams: [] (canvas shows the implicit all-node view)", () => {
    const m = model({
      nodes: [{ key: "a", type: "uml.Class", title: "A", stereotypes: [], attributes: [] }],
    });
    const g = toModelGraph(m, emptyOverlay());
    expect(g.diagrams).toEqual([]);
  });

  it("carries scalar and optional node fields straight through from the Rust node", () => {
    const m = model({
      nodes: [
        {
          key: "order",
          type: "uml.Class",
          title: "Order",
          stereotypes: ["entity"],
          abstract: true,
          description: "an order",
          attributes: [{ name: "id", type: { name: "OrderId" }, multiplicity: "1" }],
          values: ["A", "B"],
        },
      ],
    });
    const g = toModelGraph(m, emptyOverlay());
    const n = g.nodes[0];
    expect(n.stereotypes).toEqual(["entity"]);
    expect(n.abstract).toBe(true);
    expect(n.description).toBe("an order");
    expect(n.attributes[0].name).toBe("id");
    expect(n.values).toEqual(["A", "B"]);
  });
});
