import { describe, it, expect } from "vitest";
import { parseBundle } from "../src/parse";

const customers = `---
type: "OWOX Data Mart"
title: "Customers"
tags: ["owox", "view"]
---

# Customers

## Overview
- **ID:** \`abc-123\`
- **Status:** PUBLISHED
- **Definition type:** VIEW

# Schema

| Column | Type | Description |
|--------|------|-------------|
| \`id\` | INTEGER | PK. Customer id |
`;

const ordersSuperset = `---
type: "OWOX Data Mart"
title: "Orders"
tags: ["owox", "view"]
---

# Orders

## Overview
- **ID:** \`—\`
- **Status:** DRAFT
- **Definition type:** VIEW

# Schema

| Column | Type | Description |
|--------|------|-------------|
| \`order_id\` | STRING | PK. Unique order id |
| \`customer_id\` | INTEGER | FK to [Customers](./customers.md) |

## Joins

- [Customers](./customers.md) — \`customer_id = id\`
`;

describe("parseBundle (legacy OWOX format)", () => {
  it("maps a legacy mart doc onto a generic classifier with attributes", () => {
    const g = parseBundle({ "b/customers.md": customers });
    const n = g.nodes[0];
    expect(n.type).toBe("OWOX Data Mart");           // opaque token carried, renders generically
    expect(n.stereotypes).toEqual([]);
    expect(n.attributes[0]).toEqual({ name: "id", type: { name: "INTEGER" }, multiplicity: "1", description: "Customer id" });
    expect(g.diagrams).toEqual([]);
  });

  it("reads legacy joins as associates edges (keys dropped)", () => {
    const g = parseBundle({ "b/customers.md": customers, "b/orders.md": ordersSuperset });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0]).toMatchObject({ from: "orders", to: "customers", kind: "associates", bidirectional: false });
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});

const customersCard = `---
type: "OWOX Data Mart"
title: "Blocks"
tags: ["owox", "table"]
---
# Blocks
# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`hash\` | STRING | PK. id |
`;
const txCard = `---
type: "OWOX Data Mart"
title: "Transactions"
tags: ["owox", "table"]
---
# Transactions
# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`block_hash\` | STRING | PK. h |

## Joins
- [Blocks](./blocks.md) — \`block_hash = hash\` [N:1]
`;

describe("legacy cardinality suffix", () => {
  it("maps [N:1] onto per-end multiplicities", () => {
    const g = parseBundle({ "b/blocks.md": customersCard, "b/transactions.md": txCard });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].fromEnd.multiplicity).toBe("*");
    expect(g.edges[0].toEnd.multiplicity).toBe("1");
  });
  it("leaves multiplicities undefined when absent", () => {
    const txNo = txCard.replace(" [N:1]", "");
    const g = parseBundle({ "b/blocks.md": customersCard, "b/transactions.md": txNo });
    expect(g.edges[0].fromEnd.multiplicity).toBeUndefined();
  });
});
