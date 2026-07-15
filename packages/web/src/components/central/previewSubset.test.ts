import { test, expect } from "vitest";
import type { ModelNode, ModelEdge } from "@waml/okf";
import { nodePreviewSubset, edgePreviewSubset } from "./previewSubset";

const node = (key: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title: key, body: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as ModelNode;

const edge = (id: string, from: string, to: string): ModelEdge =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: {}, bidirectional: false });

// a—b, a—c, c—d. From a: keep {a,b,c}; d is reachable only via c, not directly.
const NODES = [node("a"), node("b"), node("c"), node("d")];
const EDGES = [edge("ab", "a", "b"), edge("ac", "a", "c"), edge("cd", "c", "d")];

test("node subset keeps focal + direct neighbours, drops distant nodes", () => {
  const s = nodePreviewSubset("a", NODES, EDGES);
  expect(s.nodes.map((n) => n.key).sort()).toEqual(["a", "b", "c"]);
  expect([...s.focalKeys]).toEqual(["a"]);
});

test("node subset keeps only edges with both endpoints in the kept set", () => {
  const s = nodePreviewSubset("a", NODES, EDGES);
  expect(s.edges.map((e) => e.id).sort()).toEqual(["ab", "ac"]); // cd excluded (d dropped)
});

test("edge subset keeps the edge and both endpoint nodes as focal", () => {
  const s = edgePreviewSubset("ac", NODES, EDGES);
  expect(s.nodes.map((n) => n.key).sort()).toEqual(["a", "c"]);
  expect(s.edges.map((e) => e.id)).toEqual(["ac"]);
  expect([...s.focalKeys].sort()).toEqual(["a", "c"]);
});

test("edge subset for a missing id is empty", () => {
  const s = edgePreviewSubset("nope", NODES, EDGES);
  expect(s.nodes).toEqual([]);
  expect(s.edges).toEqual([]);
  expect(s.focalKeys.size).toBe(0);
});
