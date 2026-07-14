import { describe, it, test, expect, beforeAll } from "vitest";
import type { ModelNode, ModelEdge } from "@waml/okf";
import { initWasm } from "@waml/wasm";
import { DEFAULT_DISPLAY, type DiagramDisplay } from "@waml/okf";
import { createModelStore } from "@waml/core/state/model";
import { buildRfEdges, isEdgeReconnectable, buildAnchorEdges } from "./edges";

beforeAll(async () => {
  await initWasm();
});

const node = (key: string, x = 0): ModelNode =>
  ({ concept: { id: key, type: "uml.Class", title: key, body: "" }, key, type: "uml.Class", stereotypes: [], attributes: [], position: { x, y: 0 } });
const edge = (over: Partial<ModelEdge> = {}): ModelEdge =>
  ({ id: "e1", kind: "associates", from: "a", to: "b", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, ...over });

const disp = (over: Partial<DiagramDisplay> = {}): DiagramDisplay => ({ ...DEFAULT_DISPLAY, ...over });
// Attributes hidden ⇒ compact (200×90) footprint, matching the old "compact" mode.
const compact = disp({ showAttributes: false });
const detailed = disp({ showAttributes: true });

const nodes = [node("a"), node("b", 600)];

describe("buildRfEdges", () => {
  it("one edge per model edge (floating — no fixed handle set)", () => {
    const out = buildRfEdges([edge()], nodes, compact);
    expect(out).toHaveLength(1);
    expect(out[0].id).toBe("e1");
    expect(out[0].source).toBe("a");
    expect(out[0].target).toBe("b");
    // Floating edges derive their side at render time; no handle is pinned here.
    expect(out[0].sourceHandle).toBeUndefined();
    expect(out[0].targetHandle).toBeUndefined();
    expect(out[0].type).toBe("rel");
    expect((out[0].data as { kind?: string }).kind).toBe("associates");
    expect((out[0].data as { modelEdgeId?: string }).modelEdgeId).toBe("e1");
  });

  it("still one edge per model edge when attributes are shown", () => {
    const out = buildRfEdges([edge()], nodes, detailed);
    expect(out).toHaveLength(1);
    expect(out[0].id).toBe("e1");
    expect((out[0].data as { kind?: string }).kind).toBe("associates");
    expect((out[0].data as { modelEdgeId?: string }).modelEdgeId).toBe("e1");
  });

  it("assigns an exit side per end from node geometry", () => {
    // b sits to the right of a → a exits Right, b receives on Left.
    const out = buildRfEdges([edge()], nodes, compact);
    const d = out[0].data as { sourceSide?: string; targetSide?: string };
    expect(d.sourceSide).toBe("right");
    expect(d.targetSide).toBe("left");
  });

  it("spaces edges sharing a (node, side) into distinct ordered slots", () => {
    const a = node("a"); // at x0,y0
    const hi = { ...node("b"), position: { x: 600, y: -200 } };  // right & above a's center
    const lo = { ...node("c"), position: { x: 600, y: 200 } };   // right & below
    const out = buildRfEdges(
      [edge({ id: "e1", from: "a", to: "b" }), edge({ id: "e2", from: "a", to: "c" })],
      [a, hi, lo], compact,
    );
    const d1 = out.find(e => e.id === "e1")!.data as { sourceSide?: string; sourceSlot?: { index: number; count: number } };
    const d2 = out.find(e => e.id === "e2")!.data as { sourceSide?: string; sourceSlot?: { index: number; count: number } };
    expect(d1.sourceSide).toBe("right");
    expect(d2.sourceSide).toBe("right");
    expect(d1.sourceSlot!.count).toBe(2);
    expect(d2.sourceSlot!.count).toBe(2);
    // The higher target (b, y=-200) orders first on the right side.
    expect(d1.sourceSlot!.index).toBe(0);
    expect(d2.sourceSlot!.index).toBe(1);
  });
});

describe("buildRfEdges data passthrough (driven by the active diagram's display)", () => {
  it("carries the end multiplicities and bidirectional flag", () => {
    const out = buildRfEdges([edge({ fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1" } })], nodes, compact);
    expect((out[0].data as { fromEnd?: { multiplicity?: string } }).fromEnd?.multiplicity).toBe("*");
    expect((out[0].data as { toEnd?: { multiplicity?: string } }).toEnd?.multiplicity).toBe("1");
  });

  it("threads associationLabels into edge data", () => {
    const out = buildRfEdges([edge()], nodes, disp({ associationLabels: "hidden" }));
    expect((out[0].data as { associationLabels?: string }).associationLabels).toBe("hidden");
  });

  it("reflects associationLabels 'all' (the default) into edge data", () => {
    const out = buildRfEdges([edge()], nodes, disp());
    expect((out[0].data as { associationLabels?: string }).associationLabels).toBe("all");
  });

  it("threads emphasizeMultiplicity into every edge's data", () => {
    const out = buildRfEdges([edge(), edge({ id: "e2" })], nodes, disp({ emphasizeMultiplicity: true }));
    expect(out.every(e => (e.data as { emphasizeMultiplicity?: boolean }).emphasizeMultiplicity === true)).toBe(true);
  });

  it("defaults emphasizeMultiplicity to false (per DEFAULT_DISPLAY)", () => {
    const out = buildRfEdges([edge()], nodes, disp());
    expect((out[0].data as { emphasizeMultiplicity?: boolean }).emphasizeMultiplicity).toBe(false);
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

// Integration smoke tests against the real @waml/core model store (rather than
// hand-rolled fixtures), matching how buildRfEdges/buildAnchorEdges are actually
// fed by the Svelte state bridge.
describe("integration with @waml/core's createModelStore", () => {
  test("buildRfEdges maps each model edge to one 'rel' edge with modelEdgeId in data", () => {
    const s = createModelStore();
    const a = s.addNode({ x: 0, y: 0 });
    const b = s.addNode({ x: 0, y: 0 });
    const e = s.addEdge(a.key, b.key)!;
    const { nodes: n, edges: ed } = s.get();
    const rf = buildRfEdges(ed, n, compact);
    expect(rf).toHaveLength(1);
    expect(rf[0]).toMatchObject({ id: e.id, source: a.key, target: b.key, type: "rel" });
    expect((rf[0].data as { modelEdgeId?: string }).modelEdgeId).toBe(e.id);
  });

  test("buildAnchorEdges drops connectors whose endpoints are missing", () => {
    const s = createModelStore();
    const note = s.addNode({ x: 0, y: 0 });
    s.updateNode(note.key, { type: "uml.Note" });
    const { nodes: n, edges: ed } = s.get();
    // Note annotates nothing present → no anchor edges, and never throws.
    expect(buildAnchorEdges(n, ed)).toEqual([]);
  });
});
