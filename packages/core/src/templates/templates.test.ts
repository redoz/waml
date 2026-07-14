import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, build_model } from "@waml/wasm";
import { RELATIONSHIP_KINDS } from "@waml/okf";
import { toModelGraph, emptyOverlay, type RustModel } from "../state/overlay";
import { TEMPLATES, ordersDomain } from "./index";

beforeAll(async () => {
  await initWasm();
});

const graphOf = (bundle: [string, string][]) =>
  toModelGraph(build_model(bundle) as unknown as RustModel, emptyOverlay());

describe("built-in templates", () => {
  it("ships exactly one template — Orders Domain", () => {
    expect(TEMPLATES).toHaveLength(1);
    expect(TEMPLATES[0].id).toBe("uml_orders_domain");
  });

  it("every template bundle derives a valid uml model", () => {
    for (const t of TEMPLATES) {
      const g = graphOf(t.bundle);
      for (const n of g.nodes) {
        expect(n.type).toMatch(/^uml\./);
        expect(Array.isArray(n.attributes)).toBe(true);
      }
      for (const e of g.edges) {
        expect(RELATIONSHIP_KINDS).toContain(e.kind);
      }
    }
  });
});

describe("orders-domain UML template", () => {
  it("is registered under a stable deep-link id", () => {
    expect(TEMPLATES.some((t) => t.id === "uml_orders_domain")).toBe(true);
  });

  it("uses stereotypes, an enum, composition and a diagram", () => {
    const g = graphOf(ordersDomain.bundle);
    const order = g.nodes.find((n) => n.key === "order")!;
    expect(order.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(g.nodes.find((n) => n.key === "order-status")!.values).toContain("PLACED");
    const compose = g.edges.find((e) => e.kind === "composes")!;
    expect(compose).toMatchObject({ from: "order", to: "order-line" });
    expect(g.diagrams).toHaveLength(1);
    expect(g.diagrams[0].profile).toBe("uml-domain");
    expect(g.diagrams[0].members).toContain("order");
  });

  it("attribute refs point at real member nodes", () => {
    const g = graphOf(ordersDomain.bundle);
    const keys = new Set(g.nodes.map((n) => n.key));
    for (const n of g.nodes) for (const a of n.attributes) if (a.type.ref) expect(keys.has(a.type.ref)).toBe(true);
  });
});
