import { describe, it, expect } from "vitest";
import { parseBundle } from "../src/parse";

const order = `---
type: uml.Class
stereotype: [aggregateRoot, entity]
abstract: false
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId [1]
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) [0..1]

## Relationships
- associates [Customer](./customer.md) as "places": 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)

## Glossary
Free-form section the parser does not know.
`;

const orderStatus = `---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
`;

const customer = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- associates [Order](./order.md): 1 customer to 1 order\n`;
const orderLine = `---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n`;
const pricing = `---\ntype: uml.Interface\ntitle: PricingService\n---\n# PricingService\n`;

const files = {
  "m/order.md": order, "m/order-status.md": orderStatus, "m/customer.md": customer,
  "m/order-line.md": orderLine, "m/pricing-service.md": pricing,
};

describe("parseBundle — UML format", () => {
  const g = parseBundle(files);
  const orderNode = g.nodes.find(n => n.key === "order")!;

  it("reads frontmatter type, stereotypes (list) and description", () => {
    expect(orderNode.type).toBe("uml.Class");
    expect(orderNode.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(orderNode.abstract).toBeUndefined();       // false → omitted
    expect(orderNode.description).toBe("A customer's placed order.");
  });
  it("reads scalar stereotype too", () => {
    const g2 = parseBundle({ "m/a.md": `---\ntype: uml.Class\nstereotype: entity\ntitle: A\n---\n# A\n` });
    expect(g2.nodes[0].stereotypes).toEqual(["entity"]);
  });
  it("parses attributes with refs and multiplicities", () => {
    expect(orderNode.attributes).toEqual([
      { name: "id", type: { name: "OrderId" }, multiplicity: "1" },
      { name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" },
      { name: "status", type: { name: "OrderStatus", ref: "order-status" }, multiplicity: "1" },
      { name: "shippingAddress", type: { name: "Address" }, multiplicity: "0..1" }, // no address.md → token
    ]);
  });
  it("parses enum values", () => {
    expect(g.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED"]);
  });
  it("parses relationships with kinds and ends", () => {
    const compose = g.edges.find(e => e.kind === "composes")!;
    expect(compose).toMatchObject({ from: "order", to: "order-line" });
    expect(compose.fromEnd).toEqual({ multiplicity: "1" });
    expect(compose.toEnd).toEqual({ multiplicity: "1..*", role: "lines" });
    expect(g.edges.find(e => e.kind === "depends")).toMatchObject({ from: "order", to: "pricing-service" });
  });
  it("merges reciprocal associates into one bidirectional edge (first declaration wins ends)", () => {
    const assoc = g.edges.filter(e => e.kind === "associates");
    expect(assoc).toHaveLength(1);
    expect(assoc[0]).toMatchObject({ from: "order", to: "customer", bidirectional: true, name: "places" });
    expect(assoc[0].fromEnd).toMatchObject({ multiplicity: "1", role: "order", navigable: true });
    expect(assoc[0].toEnd).toMatchObject({ multiplicity: "1", role: "customer", navigable: true });
  });
  it("one-way associates sets only the far end navigable", () => {
    const g2 = parseBundle({
      "m/a.md": `---\ntitle: A\n---\n# A\n\n## Relationships\n- associates [B](./b.md): 1 to *\n`,
      "m/b.md": `---\ntitle: B\n---\n# B\n`,
    });
    expect(g2.edges[0].fromEnd.navigable).toBeUndefined();
    expect(g2.edges[0].toEnd.navigable).toBe(true);
  });
  it("carries unknown sections on extra, never dropped", () => {
    expect(orderNode.extra).toContain("## Glossary");
    expect(orderNode.extra).toContain("Free-form section");
  });
  it("reads abstract: true", () => {
    const g2 = parseBundle({ "m/a.md": `---\ntype: uml.Class\nabstract: true\ntitle: A\n---\n# A\n` });
    expect(g2.nodes[0].abstract).toBe(true);
  });
});

describe("parseBundle — association classes (uml.Association)", () => {
  const orderAC = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md) as [Places](./places.md): 1 order to 1 customer\n`;
  const customerAC = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;
  const places = `---\ntype: uml.Association\ntitle: Places\n---\n# Places\n\n## Attributes\n- placedAt: Timestamp [1]\n- channel: [Channel](./channel.md) [1]\n`;
  const channel = `---\ntype: uml.Class\ntitle: Channel\n---\n# Channel\n`;
  const g = parseBundle({ "m/order.md": orderAC, "m/customer.md": customerAC, "m/places.md": places, "m/channel.md": channel });

  it("resolves the `as [link]` name to a { ref: nodeKey } on the edge", () => {
    const e = g.edges.find(x => x.from === "order" && x.to === "customer")!;
    expect(e.name).toEqual({ ref: "places" });
  });
  it("the association class is an ordinary classifier node with attributes — not an edge, no ends", () => {
    const ac = g.nodes.find(n => n.key === "places")!;
    expect(ac.type).toBe("uml.Association");
    expect(ac.attributes).toEqual([
      { name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" },
      { name: "channel", type: { name: "Channel", ref: "channel" }, multiplicity: "1" },
    ]);
    // The ends stay on the inline bullet; order→customer remains a direct edge.
    expect(g.edges.filter(e => e.from === "places" || e.to === "places")).toHaveLength(0);
  });
});

describe("parseBundle — notes (uml.Note)", () => {
  const noteDoc = `---\ntype: uml.Note\ntitle: Domestic-only\n---\n# Domestic-only\n\n## Body\nOnly valid for domestic customers.\n\n## Relationships\n- annotates [Order](./order.md)\n- annotates [Order](./order.md) as "places"\n`;
  const orderN = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md) as "places": 1 to 1\n\n## Notes\n- Drafts expire after 24h.\n`;
  const customerN = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;
  const g = parseBundle({ "m/domestic-only.md": noteDoc, "m/order.md": orderN, "m/customer.md": customerN });

  it("a uml.Note carries its ## Body and anchor targets (classifier + named association)", () => {
    const note = g.nodes.find(n => n.key === "domestic-only")!;
    expect(note.type).toBe("uml.Note");
    expect(note.body).toBe("Only valid for domestic customers.");
    expect(note.annotates).toEqual([
      { targetKey: "order" },
      { sourceKey: "order", name: "places" },
    ]);
    // annotates never becomes a ModelEdge.
    expect(g.edges.some(e => e.kind === "annotates")).toBe(false);
  });
  it("the ## Notes shorthand desugars to a self-anchored uml.Note node", () => {
    const shorthand = g.nodes.find(n => n.type === "uml.Note" && n.body === "Drafts expire after 24h.")!;
    expect(shorthand).toBeTruthy();
    expect(shorthand.annotates).toEqual([{ targetKey: "order" }]);
    // The host node does NOT keep ## Notes on `extra` (it desugared, not unknown-carried).
    expect(g.nodes.find(n => n.key === "order")!.extra ?? "").not.toContain("## Notes");
  });
});
