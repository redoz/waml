import { describe, it, expect } from "vitest";
import type { FlowDoc, SequenceDoc } from "@waml/okf";
import {
  toModelGraph,
  edgeKey,
  emptyOverlay,
  type Overlay,
  type RustModel,
  type RustNode,
  type RustEdge,
  type RustDiagram,
} from "./overlay";

/** A default OKF projection for fixtures that don't care about the concept tier. */
const CONCEPT = { id: "", type: "uml.Class", body: "" };

/** Node fixtures predate the additive `concept` field; the helper injects it so
 *  callers stay terse while the wire type keeps `concept` required. */
type RustNodeInput = Omit<RustNode, "concept"> & { concept?: RustNode["concept"] };

/** Diagram fixtures predate the additive `layout` field (Stage 1c, unread by
 *  `toModelGraph`); the helper injects it so callers stay terse. */
type RustDiagramInput = Omit<RustDiagram, "layout"> & { layout?: RustDiagram["layout"] };

// A minimal Rust `Model` (as serialized from wasm `build_model`) for adapter tests.
function model(partial: {
  nodes?: RustNodeInput[];
  edges?: RustEdge[];
  diagrams?: RustDiagramInput[];
  path?: string;
  packages?: RustNodeInput[];
}): RustModel {
  return {
    edges: [],
    path: "",
    ...partial,
    nodes: (partial.nodes ?? []).map((n) => ({ concept: CONCEPT, ...n })),
    packages: (partial.packages ?? []).map((n) => ({ concept: CONCEPT, ...n })),
    diagrams: (partial.diagrams ?? []).map((d) => ({ layout: [], ...d })),
  };
}

describe("toModelGraph", () => {
  it("flattens a diagram's group forest to flat members in declared, depth-first order", () => {
    const m = model({
      nodes: [
        { key: "order", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "line", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "money", type: "uml.DataType", stereotypes: [], attributes: [] },
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
        { key: "order", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", stereotypes: [], attributes: [] },
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
        { key: "order", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "customer", type: "uml.Class", stereotypes: [], attributes: [] },
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
        { key: "a", type: "uml.Class", stereotypes: [], attributes: [] },
        { key: "b", type: "uml.Class", stereotypes: [], attributes: [] },
      ],
      edges: [{ kind: "depends", from: "a", to: "b", fromEnd: {}, toEnd: {}, bidirectional: false }],
    });
    const g = toModelGraph(m, emptyOverlay());
    expect(g.edges[0].id).toBe("e1");
  });

  it("carries path and packages with members", () => {
    const m = model({
      nodes: [{ key: "order", type: "uml.Class", stereotypes: [], attributes: [] }],
      path: "acme-model",
      packages: [{ key: "", type: "uml.Package", stereotypes: [], attributes: [], members: ["order"] }],
    });
    const g = toModelGraph(m, emptyOverlay());
    expect(g.path).toBe("acme-model");
    expect(g.packages).toHaveLength(1);
    expect(g.packages[0].members).toEqual(["order"]);
  });

  it("empty diagrams yields a ModelGraph with diagrams: [] (canvas shows the implicit all-node view)", () => {
    const m = model({
      nodes: [{ key: "a", type: "uml.Class", stereotypes: [], attributes: [] }],
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
          stereotypes: ["entity"],
          abstract: true,
          attributes: [{ name: "id", type: { name: "OrderId" }, multiplicity: "1" }],
          values: ["A", "B"],
          note_body: "note prose",
          concept: { id: "shop/order", type: "uml.Class", body: "# Order\n" },
        },
      ],
    });
    const g = toModelGraph(m, emptyOverlay());
    const n = g.nodes[0];
    expect(n.stereotypes).toEqual(["entity"]);
    expect(n.abstract).toBe(true);
    expect(n.note_body).toBe("note prose");
    expect(n.attributes[0].name).toBe("id");
    expect(n.values).toEqual(["A", "B"]);
    // The nested OKF concept is forwarded straight through onto the graph node.
    expect(n.concept).toEqual({ id: "shop/order", type: "uml.Class", body: "# Order\n" });
  });

  it("passes flow views and their pools through to the ModelGraph", () => {
    const flow: FlowDoc = {
      key: "m/lifecycle",
      title: "Order Lifecycle",
      flavor: "stateMachine",
      describes: "m/order",
      nodes: ["m/lifecycle#initial", "m/lifecycle#Draft"],
      edges: ["m/lifecycle#e0"],
    };
    const activityNodes = [
      { key: "m/lifecycle#initial", id: "initial", behavior: "m/lifecycle", kind: "initial" },
      { key: "m/lifecycle#Draft", id: "Draft", behavior: "m/lifecycle", kind: "plain", entry: "reserveStock" },
    ];
    const flowEdges = [
      { key: "m/lifecycle#e0", kind: "controlFlow", behavior: "m/lifecycle", from: "m/lifecycle#initial", to: "m/lifecycle#Draft" },
    ];
    const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], flows: [flow], activityNodes, flowEdges };
    const g = toModelGraph(rust as never, emptyOverlay());
    expect(g.flows).toEqual([flow]);
    expect(g.activityNodes).toEqual(activityNodes);
    expect(g.flowEdges).toEqual(flowEdges);
  });

  it("passes sequence docs through to the ModelGraph", () => {
    const seq: SequenceDoc = {
      key: "s/place-order",
      title: "Place Order",
      lifelines: [{ title: "Customer", ref: "s/customer" }, { title: "Order", alias: "order" }],
      messages: [
        { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
        { item: "fragment", kind: "alt", operands: [{ guard: "paid", items: [] }, { items: [] }] },
      ],
    };
    const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], interactions: [seq] };
    const g = toModelGraph(rust as never, emptyOverlay());
    expect(g.interactions).toEqual([seq]);
  });
});

function modelWith(diagram: RustDiagramInput): RustModel {
  return { nodes: [], edges: [], diagrams: [{ layout: [], ...diagram }], path: "", packages: [] };
}

describe("toModelGraph diagram display/description", () => {
  it("parses stereotypeColors list into a record and copies scalars", () => {
    const g = toModelGraph(
      modelWith({
        key: "d", title: "D", profile: "uml-domain", groups: [],
        description: "Notes",
        display: { showAttributes: false, maxAttributes: 6, stereotypeColors: ["entity:#ffedd5"] },
      }),
      emptyOverlay(),
    );
    expect(g.diagrams[0].description).toBe("Notes");
    expect(g.diagrams[0].display).toEqual({ showAttributes: false, maxAttributes: 6, stereotypeColors: { entity: "#ffedd5" } });
  });

  it("splits stereotypeColors on the first colon (hex keeps its own colons? no — hex has none)", () => {
    const g = toModelGraph(
      modelWith({ key: "d", title: "D", profile: "uml-domain", groups: [], display: { stereotypeColors: ["entity:#ffedd5"] } }),
      emptyOverlay(),
    );
    expect(g.diagrams[0].display?.stereotypeColors).toEqual({ entity: "#ffedd5" });
  });

  it("leaves display undefined when the wire carries no display", () => {
    const g = toModelGraph(modelWith({ key: "d", title: "D", profile: "uml-domain", groups: [] }), emptyOverlay());
    expect(g.diagrams[0].display).toBeUndefined();
    expect(g.diagrams[0].description).toBeUndefined();
  });
});
