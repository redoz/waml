import { test, expect } from "vitest";
import { createModelStore } from "@mc/core/state/model";
import { runDagreLayout, NODE_W, NODE_H } from "./layout";

test("runDagreLayout returns a distinct position per node", () => {
  const s = createModelStore();
  const a = s.addNode({ x: 0, y: 0 });
  const b = s.addNode({ x: 0, y: 0 });
  s.addEdge(a.key, b.key);

  const { nodes, edges } = s.get();
  const positions = runDagreLayout(nodes, edges, "compact");

  expect(positions.size).toBe(2);
  expect(positions.has(a.key)).toBe(true);
  expect(positions.has(b.key)).toBe(true);
  // rankdir "LR" separates connected nodes horizontally.
  expect(positions.get(a.key)!.x).not.toBe(positions.get(b.key)!.x);
});

test("exposes the default node footprint constants", () => {
  expect(NODE_W).toBe(200);
  expect(NODE_H).toBe(90);
});
