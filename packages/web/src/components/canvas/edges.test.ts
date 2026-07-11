import { describe, it, expect } from "vitest";
import type { ModelNode, ModelEdge } from "@mc/okf";
import { buildRfEdges, isEdgeReconnectable, buildAnchorEdges } from "./edges";

const node = (key: string, x = 0): ModelNode =>
  ({ key, title: key, type: "uml.Class", stereotypes: [], attributes: [], position: { x, y: 0 } });
const edge = (over: Partial<ModelEdge> = {}): ModelEdge =>
  ({ id: "e1", kind: "associates", from: "a", to: "b", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, ...over });

const nodes = [node("a"), node("b", 600)];

describe("buildRfEdges", () => {
  it("compact: one edge per model edge using the stored handles", () => {
    const out = buildRfEdges([edge({ sourceHandle: "right", targetHandle: "left" })], nodes, "compact");
    expect(out).toHaveLength(1);
    expect(out[0].id).toBe("e1");
    expect(out[0].sourceHandle).toBe("right");
    expect(out[0].targetHandle).toBe("left");
    expect((out[0].data as { kind?: string }).kind).toBe("associates");
    expect((out[0].data as { modelEdgeId?: string }).modelEdgeId).toBe("e1");
  });

  it("erd: still one edge per model edge with associates kind + modelEdgeId", () => {
    const out = buildRfEdges([edge({ sourceHandle: "right", targetHandle: "left" })], nodes, "erd");
    expect(out).toHaveLength(1);
    expect(out[0].id).toBe("e1");
    expect((out[0].data as { kind?: string }).kind).toBe("associates");
    expect((out[0].data as { modelEdgeId?: string }).modelEdgeId).toBe("e1");
  });
});

describe("buildRfEdges geometry-derived sides (no stored handle)", () => {
  const at = (key: string, x: number): ModelNode => node(key, x);
  // Import/template edges carry no stored handle — the case that used to jump.
  const bare = edge();

  it("compact: target to the right → source exits right, target enters left", () => {
    const out = buildRfEdges([bare], [at("a", 0), at("b", 600)], "compact");
    expect(out[0].sourceHandle).toBe("right");
    expect(out[0].targetHandle).toBe("left");
  });

  it("compact: target to the left → source exits left, target enters right", () => {
    const out = buildRfEdges([bare], [at("a", 600), at("b", 0)], "compact");
    expect(out[0].sourceHandle).toBe("left");
    expect(out[0].targetHandle).toBe("right");
  });

  it("erd uses the SAME geometry side as compact (no jump on toggle)", () => {
    const out = buildRfEdges([bare], [at("a", 600), at("b", 0)], "erd");
    expect(out[0].sourceHandle).toBe("left");
    expect(out[0].targetHandle).toBe("right");
  });

  it("an explicit stored handle still wins over geometry", () => {
    const e = edge({ sourceHandle: "left", targetHandle: "right" });
    const out = buildRfEdges([e], [at("a", 0), at("b", 600)], "compact");
    expect(out[0].sourceHandle).toBe("left");
    expect(out[0].targetHandle).toBe("right");
  });
});

describe("buildRfEdges data passthrough", () => {
  it("carries the end multiplicities and bidirectional flag", () => {
    const out = buildRfEdges([edge({ fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1" } })], nodes, "compact");
    expect((out[0].data as { fromEnd?: { multiplicity?: string } }).fromEnd?.multiplicity).toBe("*");
    expect((out[0].data as { toEnd?: { multiplicity?: string } }).toEnd?.multiplicity).toBe("1");
  });

  it("threads the relLabelMode into edge data", () => {
    const out = buildRfEdges([edge()], nodes, "compact", "hidden");
    expect((out[0].data as { relLabelMode?: string }).relLabelMode).toBe("hidden");
  });

  it("defaults the mode to 'all' when the arg is omitted", () => {
    const out = buildRfEdges([edge()], nodes, "compact");
    expect((out[0].data as { relLabelMode?: string }).relLabelMode).toBe("all");
  });

  it("threads emphasizeMultiplicity=false into every edge's data", () => {
    const out = buildRfEdges([edge(), edge({ id: "e2" })], nodes, "compact", "all", false);
    expect(out.every(e => (e.data as { emphasizeMultiplicity?: boolean }).emphasizeMultiplicity === false)).toBe(true);
  });

  it("defaults emphasizeMultiplicity to true when the arg is omitted", () => {
    const out = buildRfEdges([edge()], nodes, "compact");
    expect((out[0].data as { emphasizeMultiplicity?: boolean }).emphasizeMultiplicity).toBe(true);
  });
});

describe("buildAnchorEdges (dashed connectors for association classes + notes)", () => {
  it("synthesises a dashed anchor from a uml.Association node to the association it names", () => {
    const ns: ModelNode[] = [node("order"), node("customer"), { ...node("places"), type: "uml.Association" }];
    const es: ModelEdge[] = [edge({ id: "e1", from: "order", to: "customer", name: { ref: "places" } })];
    const anchors = buildAnchorEdges(ns, es);
    expect(anchors).toEqual([{ id: "ac-e1", source: "places", target: "order", type: "anchor", selectable: false }]);
  });

  it("synthesises a dashed anchor from a uml.Note to each annotated target", () => {
    const ns: ModelNode[] = [node("order"),
      { ...node("n"), type: "uml.Note", annotates: [{ targetKey: "order" }, { sourceKey: "order", name: "places" }] }];
    const anchors = buildAnchorEdges(ns, []);
    expect(anchors.map(a => `${a.source}->${a.target}`)).toEqual(["n->order", "n->order"]);
    expect(anchors.every(a => a.type === "anchor")).toBe(true);
  });

  it("skips anchors whose endpoints are not present nodes", () => {
    const ns: ModelNode[] = [node("order"), { ...node("places"), type: "uml.Association" }];
    const es: ModelEdge[] = [edge({ id: "e1", from: "order", to: "missing", name: { ref: "places" } })];
    // target present (order) so this one lands; a note pointing at a missing key is dropped
    expect(buildAnchorEdges(ns, es)).toEqual([{ id: "ac-e1", source: "places", target: "order", type: "anchor", selectable: false }]);
    const notes: ModelNode[] = [{ ...node("n"), type: "uml.Note", annotates: [{ targetKey: "gone" }] }];
    expect(buildAnchorEdges(notes, [])).toEqual([]);
  });
});

describe("isEdgeReconnectable (only the selected relationship reconnects)", () => {
  it("is true for the selected edge", () => {
    expect(isEdgeReconnectable("e1", "e1")).toBe(true);
  });
  it("is false for a non-selected edge", () => {
    expect(isEdgeReconnectable("e2", "e1")).toBe(false);
  });
  it("is false when nothing is selected", () => {
    expect(isEdgeReconnectable("e1", null)).toBe(false);
  });
  it("is false when the edge has no modelEdgeId", () => {
    expect(isEdgeReconnectable(undefined, "e1")).toBe(false);
  });
});
