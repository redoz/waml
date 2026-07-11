import { describe, it, expect } from "vitest";
import { migrateGraph, endsFromCardinality, splitType } from "../src/index";

const legacy = {
  storageId: "stor_1",
  nodes: [{
    key: "orders", title: "Orders", inputSource: "SQL", description: "d", status: "created", owoxId: "x",
    position: { x: 5, y: 6 },
    schema: [
      { name: "id", type: "STRING", pk: true, alias: "oid", description: "Unique id" },
      { name: "total", type: "NUMERIC", pk: false },
    ],
  }],
  edges: [{ id: "e1", from: "orders", to: "customers", keys: [{ left: "customer_id", right: "id" }],
            bidirectional: false, cardinality: "N:1", sourceHandle: "right" }],
};

describe("migrateGraph", () => {
  it("maps a legacy mart graph onto the UML model", () => {
    const g = migrateGraph(legacy)!;
    expect(g.diagrams).toEqual([]);
    const n = g.nodes[0];
    expect(n).toMatchObject({ key: "orders", type: "uml.Class", title: "Orders", stereotypes: [], position: { x: 5, y: 6 } });
    expect(n.attributes).toEqual([
      { name: "id", type: { name: "STRING" }, multiplicity: "1", description: "Unique id" },
      { name: "total", type: { name: "NUMERIC" }, multiplicity: "1" },
    ]);
    expect((n as Record<string, unknown>).schema).toBeUndefined();
    const e = g.edges[0];
    expect(e).toMatchObject({ id: "e1", kind: "associates", from: "orders", to: "customers", bidirectional: false, sourceHandle: "right" });
    expect(e.fromEnd).toEqual({ multiplicity: "*" });
    expect(e.toEnd).toEqual({ multiplicity: "1", navigable: true });
  });
  it("passes a current-shape graph through and defaults missing diagrams", () => {
    const g = migrateGraph({ nodes: [], edges: [] })!;
    expect(g).toEqual({ nodes: [], edges: [], diagrams: [] });
  });
  it("returns null for garbage", () => {
    expect(migrateGraph(null)).toBeNull();
    expect(migrateGraph({ nodes: "x" })).toBeNull();
  });
  it("bidirectional legacy edges get both ends navigable", () => {
    const g = migrateGraph({ ...legacy, edges: [{ id: "e1", from: "a", to: "b", keys: [], bidirectional: true }] })!;
    expect(g.edges[0].fromEnd.navigable).toBe(true);
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});

describe("endsFromCardinality", () => {
  it("maps 1:N", () => {
    expect(endsFromCardinality("1:N", false)).toEqual({ fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*", navigable: true } });
  });
  it("no cardinality → only navigability", () => {
    expect(endsFromCardinality(undefined, false)).toEqual({ fromEnd: {}, toEnd: { navigable: true } });
  });
});

describe("splitType", () => {
  it("splits family.Metaclass", () => expect(splitType("uml.Class")).toEqual({ family: "uml", metaclass: "Class" }));
  it("rejects opaque tokens", () => {
    expect(splitType("Data Mart")).toBeNull();
    expect(splitType("uml.")).toBeNull();
    expect(splitType("noDot")).toBeNull();
  });
});
