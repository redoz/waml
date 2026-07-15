import { test, expect } from "vitest";
import type { ModelNode, ModelEdge } from "@waml/okf";
import { nodeAssociations } from "./associations";

function node(key: string, title: string): ModelNode {
  return {
    concept: { id: key, type: "uml.Class", title, body: "" },
    key,
    type: "uml.Class",
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  };
}

function edge(id: string, from: string, to: string, patch: Partial<ModelEdge> = {}): ModelEdge {
  return { id, kind: "associates", from, to, fromEnd: {}, toEnd: {}, bidirectional: false, ...patch };
}

const order = node("order", "Order");
const customer = node("customer", "Customer");
const line = node("line", "OrderLine");
const nodes = [order, customer, line];

test("collects edges on both ends and marks direction", () => {
  const edges = [
    edge("e1", "order", "line", { kind: "composes", toEnd: { multiplicity: "1..*" } }),
    edge("e2", "customer", "order"),
  ];
  const rows = nodeAssociations(order, edges, nodes);
  expect(rows).toEqual([
    { id: "e1", kind: "composes", outgoing: true, otherTitle: "OrderLine", multiplicity: "1..*", role: undefined },
    { id: "e2", kind: "associates", outgoing: false, otherTitle: "Customer", multiplicity: undefined, role: undefined },
  ]);
});

test("far end is fromEnd for incoming edges", () => {
  const edges = [edge("e1", "customer", "order", { fromEnd: { multiplicity: "0..1", role: "buyer" } })];
  const rows = nodeAssociations(order, edges, nodes);
  expect(rows[0]).toMatchObject({ outgoing: false, otherTitle: "Customer", multiplicity: "0..1", role: "buyer" });
});

test("skips unrelated edges and annotates anchors", () => {
  const edges = [
    edge("e1", "customer", "line"),
    edge("e2", "note", "order", { kind: "annotates" }),
  ];
  expect(nodeAssociations(order, edges, nodes)).toEqual([]);
});

test("falls back to the node key when the other node is missing", () => {
  const edges = [edge("e1", "order", "ghost")];
  expect(nodeAssociations(order, edges, nodes)[0].otherTitle).toBe("ghost");
});
