import { describe, it, expect } from "vitest";
import { parseBundle } from "../src/parse";
import { serializeBundle } from "../src/serialize";
import type { ModelGraph } from "../src/types";

const order = `---
type: uml.Class
stereotype: [aggregateRoot, entity]
abstract: false
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId {1}
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) {0..1}

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
  const places = `---\ntype: uml.Association\ntitle: Places\n---\n# Places\n\n## Attributes\n- placedAt: Timestamp {1}\n- channel: [Channel](./channel.md) {1}\n`;
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

describe("UML format round-trip (lossless)", () => {
  const graph: ModelGraph = {
    nodes: [
      { key: "order", type: "uml.Class", title: "Order", stereotypes: ["aggregateRoot", "entity"],
        description: "A customer's placed order.",
        attributes: [
          { name: "id", type: { name: "OrderId" }, multiplicity: "1" },
          { name: "status", type: { name: "OrderStatus", ref: "order-status" }, multiplicity: "1" },
          { name: "note", type: { name: "String" }, multiplicity: "0..1", visibility: "-" },
        ],
        position: { x: 0, y: 0 }, extra: "## Glossary\nKeep me." },
      { key: "order-line", type: "uml.Class", title: "OrderLine", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "order-status", type: "uml.Enum", title: "OrderStatus", stereotypes: [], attributes: [],
        values: ["DRAFT", "PLACED"], position: { x: 0, y: 0 } },
      { key: "base", type: "uml.Class", title: "Base", stereotypes: [], abstract: true, attributes: [], position: { x: 0, y: 0 } },
    ],
    edges: [
      { id: "e1", kind: "composes", from: "order", to: "order-line",
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1..*", role: "lines" }, bidirectional: false },
      { id: "e2", kind: "associates", from: "order", to: "customer", name: "places",
        fromEnd: { multiplicity: "1", navigable: true }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: true },
      { id: "e3", kind: "specializes", from: "order", to: "base", fromEnd: {}, toEnd: {}, bidirectional: false },
    ],
    diagrams: [],
  };

  const files = serializeBundle(graph, "Shop").files;
  const back = parseBundle(files);
  const order2 = back.nodes.find(n => n.key === "order")!;

  it("emits the spec sections", () => {
    const doc = files["shop/order.md"];
    expect(doc).toContain("stereotype: [\"aggregateRoot\", \"entity\"]");
    expect(doc).toContain("## Attributes");
    expect(doc).toContain("- id: OrderId");
    expect(doc).toContain("- status: [OrderStatus](./order-status.md)");
    expect(doc).toContain("- - note: String {0..1}");
    expect(doc).toContain("## Relationships");
    expect(doc).toContain("- composes [OrderLine](./order-line.md): 1 to 1..* lines");
    expect(doc).toContain("- specializes [Base](./base.md)");
    expect(doc).toContain("## Glossary\nKeep me.");
    expect(files["shop/order-status.md"]).toContain("## Values\n- DRAFT\n- PLACED");
    expect(files["shop/base.md"]).toContain("abstract: true");
  });
  it("bidirectional associates (with a string name) appears in BOTH docs and merges back", () => {
    expect(files["shop/order.md"]).toContain("- associates [Customer](./customer.md) as \"places\": 1 to 1");
    expect(files["shop/customer.md"]).toContain("- associates [Order](./order.md) as \"places\": 1 to 1");
    const assoc = back.edges.find(e => e.kind === "associates")!;
    expect(assoc.bidirectional).toBe(true);
    expect(assoc.name).toBe("places");
  });
  it("round-trips node substance losslessly", () => {
    expect(order2.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(order2.attributes).toEqual(graph.nodes[0].attributes);
    expect(order2.extra).toContain("## Glossary");
    expect(back.nodes.find(n => n.key === "base")!.abstract).toBe(true);
    expect(back.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED"]);
  });
  it("round-trips edge kinds and ends", () => {
    const compose = back.edges.find(e => e.kind === "composes")!;
    expect(compose.fromEnd.multiplicity).toBe("1");
    expect(compose.toEnd).toMatchObject({ multiplicity: "1..*", role: "lines" });
    expect(back.edges.find(e => e.kind === "specializes")).toMatchObject({ from: "order", to: "base" });
  });
});

describe("UML format round-trip — association classes & notes (lossless)", () => {
  const graph: ModelGraph = {
    nodes: [
      { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "places", type: "uml.Association", title: "Places", stereotypes: [],
        attributes: [{ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" }], position: { x: 0, y: 0 } },
      // A standalone note keeps its own doc because it anchors MORE THAN its host
      // (multi-target ⇒ not collapsible); the self-anchored note collapses to `## Notes`.
      { key: "domestic", type: "uml.Note", title: "Domestic-only", stereotypes: [], attributes: [],
        body: "Only valid for domestic customers.",
        annotates: [{ targetKey: "order" }, { targetKey: "customer" }], position: { x: 0, y: 0 } },
      { key: "order--note-1", type: "uml.Note", title: "Note on Order", stereotypes: [], attributes: [],
        body: "Drafts expire after 24h.", annotates: [{ targetKey: "order" }], position: { x: 0, y: 0 } },
    ],
    edges: [
      { id: "e1", kind: "associates", from: "order", to: "customer", name: { ref: "places" },
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false },
    ],
    diagrams: [],
  };
  const files = serializeBundle(graph, "Shop").files;
  const back = parseBundle(files);

  it("emits the association-class link name and re-resolves it to { ref }", () => {
    expect(files["shop/order.md"]).toContain("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1");
    expect(files["shop/places.md"]).toContain("type: \"uml.Association\"");
    expect(files["shop/places.md"]).toContain("- placedAt: Timestamp");
    expect(back.edges.find(e => e.from === "order" && e.to === "customer")!.name).toEqual({ ref: "places" });
  });
  it("emits a multi-target note's ## Body + annotates and reads it back (not collapsed)", () => {
    const note = files["shop/domestic-only.md"];   // file slug derives from the title "Domestic-only"
    expect(note).toContain("type: \"uml.Note\"");
    expect(note).toContain("## Body\nOnly valid for domestic customers.");
    expect(note).toContain("## Relationships\n- annotates [Order](./order.md)\n- annotates [Customer](./customer.md)");
    expect(back.nodes.find(n => n.type === "uml.Note" && n.title === "Domestic-only")!.annotates)
      .toEqual([{ targetKey: "order" }, { targetKey: "customer" }]);
  });
  it("collapses a self-anchored note back to `## Notes` on the host (lossless)", () => {
    // The self-anchored note is NOT emitted as its own doc; it rides on order.md.
    expect(files["shop/order--note-1.md"]).toBeUndefined();
    expect(files["shop/order.md"]).toContain("## Notes\n- Drafts expire after 24h.");
    const desugared = back.nodes.filter(n => n.type === "uml.Note" && n.body === "Drafts expire after 24h.");
    expect(desugared).toHaveLength(1);
    expect(desugared[0].annotates).toEqual([{ targetKey: "order" }]);
  });
});

describe("UML format round-trip — self-anchored note with a multi-line body (lossless)", () => {
  const body = "First line.\n- A bulleted caveat.\nThird line.";
  const graph: ModelGraph = {
    nodes: [
      { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      // Single anchor on `order`, so selfAnchorHost would collapse it — but its body is
      // multi-line, which cannot round-trip through a single `## Notes` bullet.
      { key: "order--note-1", type: "uml.Note", title: "Note on Order", stereotypes: [], attributes: [],
        body, annotates: [{ targetKey: "order" }], position: { x: 0, y: 0 } },
    ],
    edges: [],
    diagrams: [],
  };
  const files = serializeBundle(graph, "Shop").files;
  const back = parseBundle(files);

  it("keeps its own doc rather than flattening lossily into `## Notes`", () => {
    expect(files["shop/order.md"]).not.toContain("## Notes");
    expect(files["shop/note-on-order.md"]).toContain("## Body\n" + body);
    expect(files["shop/note-on-order.md"]).toContain("## Relationships\n- annotates [Order](./order.md)");
  });
  it("preserves every line of the multi-line body across serialize -> parse", () => {
    const notes = back.nodes.filter(n => n.type === "uml.Note");
    expect(notes).toHaveLength(1);
    expect(notes[0].body).toBe(body);
    expect(notes[0].annotates).toEqual([{ targetKey: "order" }]);
  });
});
