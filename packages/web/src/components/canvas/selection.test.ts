import { describe, it, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { createModelStore } from "@waml/core/state/model";
import {
  EMPTY_SELECTION,
  isSelectionEmpty,
  selectionCount,
  focusedSelection,
  accumulate,
  selectionFromFlow,
  anchorNodeIds,
  deleteSelection,
} from "./selection";

describe("selection set predicates", () => {
  it("EMPTY_SELECTION is empty", () => {
    expect(isSelectionEmpty(EMPTY_SELECTION)).toBe(true);
    expect(selectionCount(EMPTY_SELECTION)).toBe(0);
  });
  it("counts nodes + edges", () => {
    expect(isSelectionEmpty({ nodes: ["a"], edges: [] })).toBe(false);
    expect(selectionCount({ nodes: ["a", "b"], edges: ["e1"] })).toBe(3);
  });
});

describe("focusedSelection — single element drives the Inspector", () => {
  it("exactly one node → node focus", () => {
    expect(focusedSelection({ nodes: ["a"], edges: [] })).toEqual({ type: "node", id: "a" });
  });
  it("exactly one edge → edge focus", () => {
    expect(focusedSelection({ nodes: [], edges: ["e1"] })).toEqual({ type: "edge", id: "e1" });
  });
  it("multiple elements → no focus (null)", () => {
    expect(focusedSelection({ nodes: ["a", "b"], edges: [] })).toBeNull();
    expect(focusedSelection({ nodes: ["a"], edges: ["e1"] })).toBeNull();
    expect(focusedSelection(EMPTY_SELECTION)).toBeNull();
  });
});

describe("accumulate — shift/ctrl-click accumulation", () => {
  it("non-additive click replaces the set with just that element", () => {
    const s = { nodes: ["a", "b"], edges: ["e1"] };
    expect(accumulate(s, { type: "node", id: "c" }, false)).toEqual({ nodes: ["c"], edges: [] });
    expect(accumulate(s, { type: "edge", id: "e2" }, false)).toEqual({ nodes: [], edges: ["e2"] });
  });
  it("additive click ADDS a new node to the set", () => {
    const s = { nodes: ["a"], edges: [] };
    expect(accumulate(s, { type: "node", id: "b" }, true)).toEqual({ nodes: ["a", "b"], edges: [] });
  });
  it("additive click on an already-selected element toggles it OUT", () => {
    const s = { nodes: ["a", "b"], edges: [] };
    expect(accumulate(s, { type: "node", id: "a" }, true)).toEqual({ nodes: ["b"], edges: [] });
  });
  it("additive edge click accumulates edges independently of nodes", () => {
    const s = { nodes: ["a"], edges: ["e1"] };
    expect(accumulate(s, { type: "edge", id: "e2" }, true)).toEqual({ nodes: ["a"], edges: ["e1", "e2"] });
  });
});

describe("selectionFromFlow — marquee / SvelteFlow payload → model-keyed set", () => {
  it("maps enclosed nodes and edges by id", () => {
    const set = selectionFromFlow([{ id: "n1" }, { id: "n2" }], [{ id: "e1" }]);
    expect(set).toEqual({ nodes: ["n1", "n2"], edges: ["e1"] });
  });
  it("collapses ERD's multiple RF edges per model edge to one model edge id", () => {
    const set = selectionFromFlow([], [
      { id: "e1::0", data: { modelEdgeId: "e1" } },
      { id: "e1::1", data: { modelEdgeId: "e1" } },
      { id: "e2" },
    ]);
    expect(set.edges).toEqual(["e1", "e2"]);
  });
  it("de-dupes repeated ids", () => {
    const set = selectionFromFlow([{ id: "n1" }, { id: "n1" }], []);
    expect(set.nodes).toEqual(["n1"]);
  });
});

describe("anchorNodeIds — bounding-box anchors for the toolbar", () => {
  const edges = [
    { id: "e1", from: "a", to: "b" },
    { id: "e2", from: "c", to: "d" },
  ];
  it("includes selected nodes directly", () => {
    expect(anchorNodeIds({ nodes: ["a", "b"], edges: [] }, edges).sort()).toEqual(["a", "b"]);
  });
  it("adds the endpoint nodes of selected edges (edges-only selection still anchors)", () => {
    expect(anchorNodeIds({ nodes: [], edges: ["e1"] }, edges).sort()).toEqual(["a", "b"]);
  });
  it("unions nodes + edge endpoints without duplicates", () => {
    expect(anchorNodeIds({ nodes: ["a"], edges: ["e2"] }, edges).sort()).toEqual(["a", "c", "d"]);
  });
});

describe("deleteSelection — delete removes ALL selected nodes + edges", () => {
  beforeAll(async () => {
    await initWasm();
  });
  const doc = (slug: string): [string, string] => [`m/${slug}.md`, `---\ntype: uml.Class\ntitle: ${slug}\n---\n\n# ${slug}\n`];

  it("removes every selected node and edge", () => {
    const store = createModelStore([doc("a"), doc("b"), doc("c")]);
    const e = store.addEdge("a", "b");
    deleteSelection(store, { nodes: ["a", "c"], edges: e ? [e.id] : [] });
    const g = store.get();
    expect(g.nodes.map((n) => n.key).sort()).toEqual(["b"]);
    expect(g.edges).toEqual([]);
  });

  it("tolerates edges already removed as a side-effect of node removal", () => {
    const store = createModelStore([doc("a"), doc("b")]);
    const e = store.addEdge("a", "b");
    // Selecting both the node and its incident edge; removing the node drops the
    // edge first, so the explicit edge removal is a harmless no-op.
    deleteSelection(store, { nodes: ["a"], edges: e ? [e.id] : [] });
    expect(store.get().edges).toEqual([]);
    expect(store.get().nodes.map((n) => n.key)).toEqual(["b"]);
  });
});
