import { describe, it, expect } from "vitest";
import { parseBundle } from "../src/parse";

// The prose-join recovery in parse.ts builds graph edges from joins written in
// prose (the "paste messy LLM output → import" path). It is not type-related and
// must keep working. `type` is opaque, so these fixtures are minimal inline docs.

describe("join target path normalization", () => {
  it("resolves a strict ## Joins link given a nested relative path", () => {
    const files = {
      "orders.md": [
        "---", 'type: "Data Mart"', "title: Orders", "---", "",
        "# Orders", "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `customer_id` | STRING | PK. |", "",
        "## Joins", "", "- [Customers](./sub/dir/customers.md) — `customer_id = id`", "",
      ].join("\n"),
      "customers.md": [
        "---", 'type: "Data Mart"', "title: Customers", "---", "",
        "# Customers", "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `id` | STRING | PK. |", "",
      ].join("\n"),
    };
    const g = parseBundle(files);
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].keys).toEqual([{ left: "customer_id", right: "id" }]);
  });
});

describe("prose joins", () => {
  const edge = (g: ReturnType<typeof parseBundle>, a: string, b: string) =>
    g.edges.find(e => (e.from === a && e.to === b) || (e.from === b && e.to === a));

  it("recovers an edge (with key) from a join written in prose", () => {
    const files = {
      "orders.md": [
        "---", "title: Orders", "---", "",
        "# Orders", "",
        "Each order can be joined with the [Customers](./customers.md) table on `customer_id`.",
        "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `customer_id` | STRING | |", "",
      ].join("\n"),
      "customers.md": [
        "---", "title: Customers", "---", "",
        "# Customers", "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `id` | STRING | PK. |", "",
      ].join("\n"),
    };
    const g = parseBundle(files);
    const e = edge(g, "orders", "customers");
    expect(e).toBeDefined();
    expect(e!.keys.some(k => k.left === "customer_id" || k.right === "customer_id")).toBe(true);
  });

  it("recovers a keyless edge from a join link that mentions no key", () => {
    const files = {
      "inputs.md": [
        "---", "title: Inputs", "---", "",
        "# Inputs", "",
        "These rows can be joined with the [Outputs](./outputs.md) records.",
        "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `id` | STRING | PK. |", "",
      ].join("\n"),
      "outputs.md": [
        "---", "title: Outputs", "---", "",
        "# Outputs", "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `id` | STRING | PK. |", "",
      ].join("\n"),
    };
    const g = parseBundle(files);
    const e = edge(g, "inputs", "outputs");
    expect(e).toBeDefined();
    expect(e!.keys).toEqual([]);
  });

  it("does not invent edges when a join link points at a non-node index.md", () => {
    const files = {
      "events.md": [
        "---", "title: Events", "---", "",
        "# Events", "",
        "This table can be joined with the reference in [references](./references/index.md).",
        "", "## Schema", "", "| Column | Type | Description |",
        "|--------|------|-------------|", "| `id` | STRING | PK. |", "",
      ].join("\n"),
      "references/index.md": [
        "---", "type: index", "title: References", "---", "", "# References", "",
      ].join("\n"),
    };
    const g = parseBundle(files);
    expect(g.edges).toHaveLength(0);
  });
});
