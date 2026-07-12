import { describe, it, expect } from "vitest";
import { RELATIONSHIP_KINDS } from "@uaml/okf";
import { TEMPLATES } from "./index";
import { ordersDomain } from "./orders-domain";

describe("built-in templates", () => {
  it("every template graph is new-shape", () => {
    for (const t of TEMPLATES) {
      expect(Array.isArray(t.graph.diagrams)).toBe(true);
      for (const n of t.graph.nodes) {
        expect(n.type).toMatch(/^uml\./);
        expect(Array.isArray(n.attributes)).toBe(true);
        expect((n as unknown as Record<string, unknown>).schema).toBeUndefined();
      }
      for (const e of t.graph.edges) {
        expect(RELATIONSHIP_KINDS).toContain(e.kind);
        expect((e as unknown as Record<string, unknown>).keys).toBeUndefined();
      }
    }
  });
  it("ships exactly one template — Orders Domain", () => {
    expect(TEMPLATES).toHaveLength(1);
    expect(TEMPLATES[0].id).toBe("uml_orders_domain");
  });
});

describe("orders-domain UML template", () => {
  it("is registered under a stable deep-link id", () => {
    expect(TEMPLATES.some(t => t.id === "uml_orders_domain")).toBe(true);
  });
  it("uses stereotypes, an enum, composition and a diagram", () => {
    const g = ordersDomain.graph;
    const order = g.nodes.find(n => n.key === "order")!;
    expect(order.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(g.nodes.find(n => n.key === "order-status")!.values).toContain("PLACED");
    const compose = g.edges.find(e => e.kind === "composes")!;
    expect(compose).toMatchObject({ from: "order", to: "order-line" });
    expect(g.diagrams).toHaveLength(1);
    expect(g.diagrams[0].profile).toBe("uml-domain");
    expect(g.diagrams[0].members).toContain("order");
  });
  it("attribute refs point at real member nodes", () => {
    const g = ordersDomain.graph;
    const keys = new Set(g.nodes.map(n => n.key));
    for (const n of g.nodes) for (const a of n.attributes)
      if (a.type.ref) expect(keys.has(a.type.ref)).toBe(true);
  });
});
