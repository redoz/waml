import { describe, it, expect } from "vitest";
import { effectiveDiagrams, ALL_DIAGRAM_KEY } from "./diagrams";
import { createModelStore } from "./model";
import type { ModelGraph } from "@mc/okf";

const node = (key: string): ModelGraph["nodes"][0] =>
  ({ key, title: key, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

describe("effectiveDiagrams", () => {
  it("empty diagrams ⇒ one implicit All diagram with every node", () => {
    const g: ModelGraph = { nodes: [node("a"), node("b")], edges: [], diagrams: [] };
    const d = effectiveDiagrams(g);
    expect(d).toHaveLength(1);
    expect(d[0]).toMatchObject({ key: ALL_DIAGRAM_KEY, profile: "uml-domain", members: ["a", "b"] });
  });
  it("explicit diagrams pass through untouched", () => {
    const g: ModelGraph = { nodes: [node("a")], edges: [], diagrams: [{ key: "d1", title: "D", profile: "p", members: ["a"] }] };
    expect(effectiveDiagrams(g)).toEqual(g.diagrams);
  });
  it("returns a referentially stable result for the same graph (implicit All)", () => {
    // Canvas passes this into effect deps; a fresh object each call would re-fire
    // the setRfNodes effect every render, leaving React Flow nodes visibility:hidden.
    const g: ModelGraph = { nodes: [node("a"), node("b")], edges: [], diagrams: [] };
    expect(effectiveDiagrams(g)).toBe(effectiveDiagrams(g));
  });
});

describe("store diagram CRUD", () => {
  it("addDiagram seeds members with current nodes; addNode joins the active diagram", () => {
    const store = createModelStore({ nodes: [node("n1")], edges: [], diagrams: [] });
    const d = store.addDiagram("Core");
    expect(d.members).toEqual(["n1"]);
    const n = store.addNode({ x: 0, y: 0 }, d.key);
    expect(store.get().diagrams[0].members).toContain(n.key);
  });
  it("removeDiagram deletes only the view", () => {
    const store = createModelStore({ nodes: [node("n1")], edges: [], diagrams: [] });
    const d = store.addDiagram("Core");
    store.removeDiagram(d.key);
    expect(store.get().diagrams).toEqual([]);
    expect(store.get().nodes).toHaveLength(1);
  });
});
