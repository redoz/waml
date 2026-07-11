import { describe, it, expect } from "vitest";
import { filesToGraph } from "./io";

// The exact worked example from public/okf-format.md (the AI authoring guide).
// This guards that the documented format — especially the Joins syntax — keeps
// importing correctly. If the parser or the guide drift, this fails.
const GUIDE_EXAMPLE = `
<!-- shop/customers.md -->
---
type: "OWOX Data Mart"
title: "Customers"
description: "One row per customer"
tags: ["owox", "view"]
---
# Customers

## Overview
- **ID:** \`—\`
- **Status:** DRAFT
- **Definition type:** VIEW
- **Storage:** —

# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`id\` | STRING | PK. Unique customer id |
| \`email\` | STRING | Contact email |
| \`country\` | STRING | Billing country |

<!-- shop/products.md -->
---
type: "OWOX Data Mart"
title: "Products"
description: "One row per product"
tags: ["owox", "view"]
---
# Products

## Overview
- **ID:** \`—\`
- **Status:** DRAFT
- **Definition type:** VIEW
- **Storage:** —

# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`id\` | STRING | PK. Unique product id |
| \`name\` | STRING | Product name |
| \`unit_price\` | NUMERIC | List price per unit |

<!-- shop/orders.md -->
---
type: "OWOX Data Mart"
title: "Orders"
description: "One row per order"
tags: ["owox", "sql"]
---
# Orders

## Overview
- **ID:** \`—\`
- **Status:** DRAFT
- **Definition type:** SQL
- **Storage:** —

# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`id\` | STRING | PK. Unique order id |
| \`customer_id\` | STRING | FK to [Customers](./customers.md) |
| \`order_date\` | DATE | Date the order was placed |
| \`total\` | NUMERIC | Order total, gross |

## Joins
- [Customers](./customers.md) — \`customer_id = id\` [N:1]

<!-- shop/order-items.md -->
---
type: "OWOX Data Mart"
title: "Order Items"
description: "One row per order line item"
tags: ["owox", "view"]
---
# Order Items

## Overview
- **ID:** \`—\`
- **Status:** DRAFT
- **Definition type:** VIEW
- **Storage:** —

# Schema
| Column | Type | Description |
|--------|------|-------------|
| \`id\` | STRING | PK. Unique line-item id |
| \`order_id\` | STRING | FK to [Orders](./orders.md) |
| \`product_id\` | STRING | FK to [Products](./products.md) |
| \`quantity\` | INTEGER | Units purchased |
| \`unit_price\` | NUMERIC | Price charged per unit |

## Joins
- [Orders](./orders.md) — \`order_id = id\` [N:1]
- [Products](./products.md) — \`product_id = id\` [N:1]
`;

describe("legacy mart format still imports", () => {
  const graph = filesToGraph({ "pasted.md": GUIDE_EXAMPLE });

  it("parses 4 marts", () => {
    expect(graph.nodes.map(n => n.key).sort()).toEqual(["customers", "order-items", "orders", "products"]);
  });

  it("parses all 3 relationships with correct direction", () => {
    const j = graph.edges.map(e => `${e.from}->${e.to}:${e.kind}`).sort();
    expect(j).toEqual([
      "order-items->orders:associates",
      "order-items->products:associates",
      "orders->customers:associates",
    ]);
  });

  it("parses the [N:1] suffix onto end multiplicities", () => {
    const e = graph.edges.find(x => x.from === "order-items" && x.to === "orders")!;
    expect(e.fromEnd.multiplicity).toBe("*");
    expect(e.toEnd.multiplicity).toBe("1");
  });
});

// The exact UML worked example from public/okf-format.md. It MUST stay
// character-identical with the guide's fenced example — that's the drift guard
// that keeps the documented format executable. If you edit one, edit both.
const UML_GUIDE_EXAMPLE = `
<!-- shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId {1}
- status: [OrderStatus](./order-status.md) {1}
- total: [Money](./money.md) {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines

<!-- shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int {1}
- unitPrice: [Money](./money.md) {1}

<!-- shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId {1}
- name: String {1}

<!-- shop/order-status.md -->
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED

<!-- shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal {1}
- currency: CurrencyCode {1}

<!-- shop/orders-domain.md -->
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
`;

describe("okf authoring guide — UML worked example imports", () => {
  const graph = filesToGraph({ "pasted.md": UML_GUIDE_EXAMPLE });

  it("parses 5 classifiers and 1 diagram", () => {
    expect(graph.nodes.map(n => n.key).sort()).toEqual(["customer", "money", "order", "order-line", "order-status"]);
    expect(graph.diagrams).toHaveLength(1);
    expect(graph.diagrams[0].members).toHaveLength(5);
  });
  it("stereotypes, refs, enum values and kinds all land", () => {
    const order = graph.nodes.find(n => n.key === "order")!;
    expect(order.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(order.attributes.find(a => a.name === "total")!.type).toEqual({ name: "Money", ref: "money" });
    expect(graph.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED", "SHIPPED", "CANCELLED"]);
    expect(graph.edges.map(e => e.kind).sort()).toEqual(["associates", "composes"]);
    const compose = graph.edges.find(e => e.kind === "composes")!;
    expect(compose.toEnd).toMatchObject({ multiplicity: "1..*", role: "lines" });
  });
});
