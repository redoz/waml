import { describe, it, expect } from "vitest";
import { parseBundle, serializeBundle, DEFAULT_DISPLAY, resolveDisplay } from "../src/index";
import type { ModelGraph, DiagramDisplay } from "../src/types";

const diagram = `---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Members
- [Order](./order.md) at 40,80
- [Customer](./customer.md)

## Render hints
- emphasize: multiplicity, composition
- collapse [Customer](./customer.md)
`;
const order = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n`;
const customer = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;

describe("diagram docs", () => {
  const g = parseBundle({ "m/orders-domain-model.md": diagram, "m/order.md": order, "m/customer.md": customer });

  it("a Diagram doc is not a node", () => {
    expect(g.nodes.map(n => n.key).sort()).toEqual(["customer", "order"]);
  });
  it("members resolve to node keys in order; profile read from frontmatter", () => {
    expect(g.diagrams).toHaveLength(1);
    expect(g.diagrams[0]).toMatchObject({
      key: "orders-domain-model", title: "Orders Domain Model", profile: "uml-domain",
      members: ["order", "customer"],
    });
  });
  it("render hints: emphasize + collapse", () => {
    expect(g.diagrams[0].hints).toEqual({ emphasize: ["multiplicity", "composition"], collapse: ["customer"] });
  });
  it("member `at x,y` lands on node.position", () => {
    expect(g.nodes.find(n => n.key === "order")!.position).toEqual({ x: 40, y: 80 });
  });
  it("a diagram doc without a display block resolves to DEFAULT_DISPLAY", () => {
    // The fixture diagram declares no `display`, so it stays absent on the model…
    expect(g.diagrams[0].display).toBeUndefined();
    // …and resolves to the defaults at render time.
    expect(resolveDisplay(g.diagrams[0].display)).toEqual(DEFAULT_DISPLAY);
  });

  it("round-trips diagrams", () => {
    const graph: ModelGraph = {
      nodes: [
        { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 12, y: 34 } },
        { key: "money", type: "uml.DataType", title: "Money", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      ],
      edges: [],
      diagrams: [{ key: "core", title: "Core", profile: "uml-domain", members: ["order", "money"], hints: { collapse: ["money"] } }],
    };
    const files = serializeBundle(graph, "Shop").files;
    expect(files["shop/core.md"]).toContain("type: \"Diagram\"");
    expect(files["shop/core.md"]).toContain("- [Order](./order.md) at 12,34");
    expect(files["shop/core.md"]).toContain("- collapse [Money](./money.md)");
    const back = parseBundle(files);
    expect(back.diagrams).toHaveLength(1);
    expect(back.diagrams[0].members).toEqual(["order", "money"]);
    expect(back.diagrams[0].hints).toEqual({ collapse: ["money"] });
    expect(back.nodes.find(n => n.key === "order")!.position).toEqual({ x: 12, y: 34 });
  });

  it("round-trips a diagram's per-diagram display settings", () => {
    const display: DiagramDisplay = {
      showAttributes: false,
      attributeDetail: "name-only",
      associationLabels: "hidden",
      emphasizeMultiplicity: true,
      showStereotype: false,
    };
    const graph: ModelGraph = {
      nodes: [{ key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } }],
      edges: [],
      diagrams: [{ key: "core", title: "Core", profile: "uml-domain", members: ["order"], display }],
    };
    const files = serializeBundle(graph, "Shop").files;
    const back = parseBundle(files);
    expect(back.diagrams[0].display).toEqual(display);
  });

  it("leaves display absent when the source diagram had none", () => {
    const graph: ModelGraph = {
      nodes: [{ key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } }],
      edges: [],
      diagrams: [{ key: "core", title: "Core", profile: "uml-domain", members: ["order"] }],
    };
    const files = serializeBundle(graph, "Shop").files;
    expect(files["shop/core.md"]).not.toContain("display:");
    expect(parseBundle(files).diagrams[0].display).toBeUndefined();
  });
});
