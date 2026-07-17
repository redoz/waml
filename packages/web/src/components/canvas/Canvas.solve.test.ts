import { test, expect, beforeAll, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import { initWasm } from "@waml/wasm";
import Canvas from "./Canvas.svelte";
import { store } from "../../state/model.svelte";

// A real Diagram doc (has a `## Layout` section) + its three member classes.
const solvedBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];

beforeAll(async () => {
  await initWasm();
});
beforeEach(() => {
  // The store + persisted active-diagram key are module singletons shared across
  // tests; reset both so each test starts from a known view.
  localStorage.clear();
  store.load([]);
});

test("a real Diagram view solves: member positions come from the solver, not the origin", async () => {
  store.load(solvedBundle);
  render(Canvas);
  await tick();
  await tick();
  const g = store.get();
  const order = g.nodes.find((n) => n.key === "shop/order")!;
  // Solver top-left for Order with the canvas's real erdAwareNodeSize widths
  // (wider than the unit test's fixed w:200, so Order sits further right) — the
  // point is it comes from the solver, not the origin.
  expect(order.position).toEqual({ x: 314, y: 69 });
});

test("the implicit All view falls back to dagre (no solve)", async () => {
  // No Diagram doc → effectiveDiagrams synthesizes the "All" view → dagre.
  store.load([
    ["a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"],
    ["b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"],
  ]);
  render(Canvas);
  await tick();
  await tick();
  const g = store.get();
  // dagre laid the two nodes out at distinct, non-origin positions.
  const a = g.nodes.find((n) => n.key === "a")!;
  const b = g.nodes.find((n) => n.key === "b")!;
  expect(a.position).not.toEqual(b.position);
});

const badRefBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### All\n- [Customer](./customer.md)\n- [Order](./order.md)\n\n## Layout\n- Ghosts left of Order\n",
  ],
];

test("a diagram referencing a non-member surfaces the diagnostics banner, and it dismisses", async () => {
  store.load(badRefBundle);
  const { getByRole, queryByRole } = render(Canvas);
  await tick();
  await tick();
  const banner = getByRole("alert");
  expect(banner.textContent).toMatch(/Ghosts/i);
  await fireEvent.click(getByRole("button", { name: /dismiss layout warnings/i }));
  await tick();
  expect(queryByRole("alert")).toBeNull();
});

test("a dragged position is a free override: it persists until the next solve trigger", async () => {
  store.load(solvedBundle);
  render(Canvas);
  await tick();
  await tick();
  // Sanity: the solve placed Order at its golden spot (real erdAwareNodeSize widths).
  expect(store.get().nodes.find((n) => n.key === "shop/order")!.position).toEqual({ x: 314, y: 69 });

  // Simulate a drag drop write-back (what onNodeDragStop does).
  store.updateNode("shop/order", { position: { x: 999, y: 999 } });
  await tick();
  await tick();

  // No reactive effect re-solves it back — the override stands until an explicit
  // solve trigger (view switch / layout button / load) overwrites it.
  expect(store.get().nodes.find((n) => n.key === "shop/order")!.position).toEqual({ x: 999, y: 999 });
});
