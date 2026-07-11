import { describe, it, expect } from "vitest";
import { serializeBundle } from "../src/serialize";
import type { ModelGraph } from "../src/types";

const graph: ModelGraph = {
  nodes: [
    { key: "orders", title: "Orders", type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 },
      attributes: [
        { name: "order_id", type: { name: "STRING" }, multiplicity: "1", description: "Unique order id" },
        { name: "customer_id", type: { name: "INTEGER" }, multiplicity: "1" },
      ] },
    { key: "customers", title: "Customers", type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 },
      attributes: [{ name: "id", type: { name: "INTEGER" }, multiplicity: "1" }] },
  ],
  edges: [{ id: "e1", kind: "associates", from: "orders", to: "customers",
            fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false }],
  diagrams: [],
};

describe("serializeBundle (interim legacy emission)", () => {
  const { files } = serializeBundle(graph, "Demo");
  const index = files["demo/index.md"];
  const orders = files["demo/orders.md"];

  it("writes a folder bundle with index + per-doc files", () => {
    expect(Object.keys(files).sort()).toEqual(["demo/customers.md", "demo/index.md", "demo/orders.md"]);
  });
  it("index lists documents with their type", () => {
    expect(index).toContain("| Document | Type |");
    expect(index).toContain("[Orders](./orders.md) | uml.Class |");
    expect(index).not.toContain("owox");
  });
  it("doc frontmatter carries the node type verbatim", () => {
    expect(orders).toContain(`type: "uml.Class"`);
    expect(orders).not.toContain("owox:");
    expect(orders).not.toContain("tags:");
    expect(orders).not.toContain("## Overview");
  });
  it("schema table renders from attributes", () => {
    expect(orders).toContain("# Schema\n\n| Column | Type | Description |");
    expect(orders).toContain("| `order_id` | STRING | Unique order id |");
  });
  it("joins render with a cardinality suffix from end multiplicities", () => {
    expect(orders).toContain("## Joins");
    expect(orders).toContain("- [Customers](./customers.md) [N:1]");
  });
});
