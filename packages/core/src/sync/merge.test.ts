import { describe, it, expect } from "vitest";
import { mergeGraphs } from "./merge";
import type { ModelGraph } from "@uaml/okf";

const node = (key: string, title: string): ModelGraph["nodes"][0] =>
  ({ concept: { id: key, type: "uml.Class", title, body: "" }, key, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge = (id: string, from: string, to: string): ModelGraph["edges"][0] =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false });

describe("mergeGraphs", () => {
  it("appends incoming nodes with fresh keys and remaps edges + diagram members", () => {
    const current: ModelGraph = { nodes: [node("n1", "A")], edges: [], diagrams: [], path: "", packages: [] };
    const incoming: ModelGraph = {
      nodes: [node("n1", "B"), node("n2", "C")],
      edges: [edge("e1", "n1", "n2")],
      diagrams: [{ key: "d", title: "D", profile: "uml-domain", members: ["n1", "n2"] }],
      path: "",
      packages: [],
    };
    const { graph, newKeys } = mergeGraphs(current, incoming);
    expect(graph.nodes).toHaveLength(3);
    expect(new Set(graph.nodes.map(n => n.key)).size).toBe(3);
    expect(newKeys.size).toBe(2);
    const merged = graph.edges[0];
    expect(newKeys.has(merged.from)).toBe(true);
    expect(newKeys.has(merged.to)).toBe(true);
    expect(graph.diagrams[0].members.every(k => newKeys.has(k))).toBe(true);
  });
});
