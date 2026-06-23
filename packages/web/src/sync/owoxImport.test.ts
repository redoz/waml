import { describe, it, expect } from "vitest";
import { selectMartIds, payloadToGraph, mergeGraphs, type ImportPayload } from "./owoxImport";
import type { ModelGraph } from "@mc/okf";

const mart = (id: string, status: string) => ({ id, title: id, status, schema: [], inputSource: "SQL" as const, definition: null });
const base: ImportPayload = {
  storageId: "st_1", total: 3, truncated: false,
  marts: [mart("a", "PUBLISHED"), mart("b", "DRAFT"), mart("c", "DRAFT")],
  relationships: [
    { sourceId: "a", targetId: "b", joinConditions: [{ sourceFieldName: "bid", targetFieldName: "id" }] },
    { sourceId: "b", targetId: "a", joinConditions: [{ sourceFieldName: "id", targetFieldName: "bid" }] },
  ],
};

describe("selectMartIds", () => {
  it("all → every mart", () => expect(selectMartIds(base, "all")).toEqual(new Set(["a", "b", "c"])));
  it("published → only published, then pulls related partners", () =>
    // "a" is published; it relates to "b", so pull-partner adds "b". "c" stays out.
    expect(selectMartIds(base, "published")).toEqual(new Set(["a", "b"])));
  it("with-relationships → only marts that have a relationship", () =>
    expect(selectMartIds(base, "with-relationships")).toEqual(new Set(["a", "b"])));
});

describe("payloadToGraph", () => {
  it("marts become created nodes with owoxId", () => {
    const g = payloadToGraph(base, "all");
    const a = g.nodes.find(n => n.owoxId === "a")!;
    expect(a.status).toBe("created");
    expect(g.storageId).toBe("st_1");
  });
  it("collapses A→B and B→A into one bidirectional edge", () => {
    const g = payloadToGraph(base, "all");
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].bidirectional).toBe(true);
  });
  it("drops edges to marts outside the selection", () => {
    const g = payloadToGraph(base, "published"); // a,b selected; both rel ends in set → 1 edge
    expect(g.nodes.map(n => n.owoxId).sort()).toEqual(["a", "b"]);
    expect(g.edges).toHaveLength(1);
  });
});

describe("mergeGraphs", () => {
  it("updates a node with a matching owoxId in place (keeps position) and adds new ones", () => {
    const current: ModelGraph = { storageId: "st_1", nodes: [
      { key: "n1", title: "old", inputSource: "SQL", schema: [], position: { x: 50, y: 60 }, status: "created", owoxId: "a" },
    ], edges: [] };
    const incoming = payloadToGraph(base, "all");
    const { graph, newKeys } = mergeGraphs(current, incoming);
    const a = graph.nodes.find(n => n.owoxId === "a")!;
    expect(a.key).toBe("n1");                 // kept the existing node
    expect(a.position).toEqual({ x: 50, y: 60 }); // kept its position
    expect(graph.nodes.some(n => n.owoxId === "b")).toBe(true); // new node added
    expect(newKeys.has(a.key)).toBe(false);
  });
});
