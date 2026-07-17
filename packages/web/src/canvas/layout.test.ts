import { test, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { DEFAULT_DISPLAY } from "@waml/okf";
import { createModelStore } from "@waml/core/state/model";
import { runDagreLayout, runSolveLayout, NODE_W, NODE_H } from "./layout";

beforeAll(async () => {
  await initWasm();
});

test("runDagreLayout returns a distinct position per node", () => {
  const s = createModelStore();
  const a = s.addNode({ x: 0, y: 0 });
  const b = s.addNode({ x: 0, y: 0 });
  s.addEdge(a.key, b.key);

  const { nodes, edges } = s.get();
  const positions = runDagreLayout(nodes, edges, DEFAULT_DISPLAY);

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

// Mirrors packages/wasm/src/solve.test.ts: a 3-class shop diagram whose
// `## Layout` prose frames the "Users" group and places it left of "Orders".
const solveBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];
const solveSizes = {
  "shop/customer": { w: 200, h: 90 },
  "shop/account": { w: 200, h: 90 },
  "shop/order": { w: 200, h: 90 },
};

test("runSolveLayout returns top-left positions, a Frame group, and no diagnostics", () => {
  const r = runSolveLayout(solveBundle, "shop/orders", solveSizes);
  expect(r.diagnostics).toEqual([]);
  // Rect.x/y is already the top-left — no centering fix-up like dagre.
  expect(r.positions.get("shop/customer")).toEqual({ x: 16, y: 16 });
  expect(r.positions.get("shop/account")).toEqual({ x: 16, y: 122 });
  expect(r.positions.get("shop/order")).toEqual({ x: 264, y: 69 });
  expect(r.groups.some((g) => g.shape === "Frame" && g.title === "Users")).toBe(true);
});

test("runSolveLayout maps a `collapsed` flag onto the node's key", () => {
  // `collapsed` is a real layout-prose flag (crates/waml/src/layout.rs L323, L673).
  const collapsing = solveBundle.map(
    ([p, t]) => (p === "shop/orders.md" ? [p, t + "- Order with collapsed\n"] : [p, t]) as [string, string],
  );
  const r = runSolveLayout(collapsing, "shop/orders", solveSizes);
  expect(Object.values(r.flags).some((f) => f.collapsed)).toBe(true);
});

test("runSolveLayout surfaces an unresolved-layout-ref diagnostic", () => {
  const bad = solveBundle.map(
    ([p, t]) => (p === "shop/orders.md" ? [p, t + "- Ghosts left of Orders\n"] : [p, t]) as [string, string],
  );
  const r = runSolveLayout(bad, "shop/orders", solveSizes);
  expect(r.diagnostics.some((d) => d.code === "unresolved-layout-ref")).toBe(true);
});
