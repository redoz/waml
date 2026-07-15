import { describe, it, expect } from "vitest";
import { effectiveDiagrams, defaultDiagramKey, ALL_DIAGRAM_KEY } from "./diagrams";
import type { ModelGraph } from "@waml/okf";

const node = (key: string): ModelGraph["nodes"][0] =>
  ({ concept: { id: key, type: "uml.Class", title: key, body: "" }, key, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

// NOTE: diagram *editing* is derived-only in Stage 1b (no diagram / membership
// ops); the store's diagram mutators are no-ops until Stage 1c. `effectiveDiagrams`
// stays the read-side contract the canvas relies on.
describe("effectiveDiagrams", () => {
  it("empty diagrams ⇒ one implicit All diagram with every node", () => {
    const g: ModelGraph = { nodes: [node("a"), node("b")], edges: [], diagrams: [], path: "", packages: [] };
    const d = effectiveDiagrams(g);
    expect(d).toHaveLength(1);
    expect(d[0]).toMatchObject({ key: ALL_DIAGRAM_KEY, profile: "uml-domain", members: ["a", "b"] });
  });
  it("explicit diagrams pass through untouched", () => {
    const g: ModelGraph = { nodes: [node("a")], edges: [], diagrams: [{ key: "d1", title: "D", profile: "p", members: ["a"] }], path: "", packages: [] };
    expect(effectiveDiagrams(g)).toEqual(g.diagrams);
  });
  it("returns a referentially stable result for the same graph (implicit All)", () => {
    // Canvas passes this into effect deps; a fresh object each call would re-fire
    // the setRfNodes effect every render, leaving React Flow nodes visibility:hidden.
    const g: ModelGraph = { nodes: [node("a"), node("b")], edges: [], diagrams: [], path: "", packages: [] };
    expect(effectiveDiagrams(g)).toBe(effectiveDiagrams(g));
  });
});

describe("defaultDiagramKey", () => {
  it("explicit diagrams win over flows and interactions", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], path: "", packages: [],
      diagrams: [{ key: "d1", title: "D", profile: "p", members: ["a"] }],
      flows: [{ key: "f1", title: "F", flavor: "activity", nodes: [], edges: [] }] as ModelGraph["flows"],
      interactions: [{ key: "s1", title: "S", lifelines: [], messages: [] }] as ModelGraph["interactions"],
    };
    expect(defaultDiagramKey(g)).toBe("d1");
  });
  it("no diagrams, has flows ⇒ first flow key", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [],
      flows: [{ key: "f1", title: "F", flavor: "activity", nodes: [], edges: [] }] as ModelGraph["flows"],
    };
    expect(defaultDiagramKey(g)).toBe("f1");
  });
  it("no diagrams or flows, has interactions ⇒ first interaction key", () => {
    const g: ModelGraph = {
      nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [],
      interactions: [{ key: "s1", title: "S", lifelines: [], messages: [] }] as ModelGraph["interactions"],
    };
    expect(defaultDiagramKey(g)).toBe("s1");
  });
  it("no diagrams/flows/interactions ⇒ synthetic All key", () => {
    const g: ModelGraph = { nodes: [node("a")], edges: [], diagrams: [], path: "", packages: [] };
    expect(defaultDiagramKey(g)).toBe(ALL_DIAGRAM_KEY);
  });
});
