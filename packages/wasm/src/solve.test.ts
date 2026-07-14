// Parity vitest for `solve()`: proves the JS wasm bridge reproduces the
// Rust golden layout from `crates/waml/tests/solver_golden.rs` (also
// exercised natively in `crates/waml-wasm/tests/native.rs`).
import { beforeAll, describe, expect, it } from "vitest";
import { initWasm, solve } from "./index";

const bundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    // `Diagram.key` is derived from the last path segment (`doc_slug`), so
    // this must be `orders.md`, not `orders-domain.md`, to resolve to key
    // "orders" — matching the golden fixture.
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];

const sizes = {
  customer: { w: 200, h: 90 },
  account: { w: 200, h: 90 },
  order: { w: 200, h: 90 },
};

describe("solve() over wasm", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("returns the golden rects as plain objects", () => {
    const { solved, diagnostics } = solve(bundle, "orders", sizes);
    expect(diagnostics).toEqual([]);
    // Plain object, not a Map.
    expect(solved.nodes.customer).toEqual({ x: 16, y: 16, w: 200, h: 90 });
    expect(solved.nodes.account).toEqual({ x: 16, y: 122, w: 200, h: 90 });
    expect(solved.nodes.order).toEqual({ x: 264, y: 69, w: 200, h: 90 });
    expect(solved.groups).toHaveLength(2);
    // The framed "Users" group renders with a title.
    expect(solved.groups.some((g) => g.title === "Users")).toBe(true);
  });

  it("throws when the diagram key is unknown", () => {
    expect(() => solve(bundle, "nope", sizes)).toThrow(/nope/);
  });

  it("surfaces an unresolved-operand diagnostic", () => {
    const bad = bundle.map(
      ([p, t]) =>
        (p === "shop/orders.md"
          ? [p, t + "- Ghosts left of Orders\n"]
          : [p, t]) as [string, string],
    );
    const { diagnostics } = solve(bad, "orders", sizes);
    expect(diagnostics.some((d) => d.code === "unresolved-layout-ref")).toBe(true);
  });
});
