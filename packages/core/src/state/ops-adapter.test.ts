import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, apply_ops, build_model } from "@uaml/wasm";
import { toModelGraph, emptyOverlay, type RustModel } from "./overlay";
import {
  nodeNewOps,
  nodeRenameOps,
  nodeRmOps,
  nodeSetOps,
  attrDiffOps,
  valueDiffOps,
  updateNodeOps,
  edgeAddOps,
  edgeSetOps,
  edgeRmOps,
  type OpDto,
} from "./ops-adapter";
import type { ModelGraph, Attribute } from "@uaml/okf";

type Bundle = [string, string][];

const BASE: Bundle = [
  [
    "m/order.md",
    "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money {0..1}\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n",
  ],
  ["m/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["m/status.md", "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n"],
];

function derive(bundle: Bundle): ModelGraph {
  return toModelGraph(build_model(bundle) as unknown as RustModel, emptyOverlay());
}
function apply(bundle: Bundle, ops: OpDto[]): ModelGraph {
  return derive(apply_ops(bundle, ops) as Bundle);
}

let base: ModelGraph;
beforeAll(async () => {
  await initWasm();
  base = derive(BASE);
});

const orderNode = () => base.nodes.find((n) => n.key === "order")!;
const statusNode = () => base.nodes.find((n) => n.key === "status")!;
const oc = () => base.edges.find((e) => e.from === "order" && e.to === "customer")!;

describe("node ops", () => {
  it("scalar edit → a single node.set carrying only the changed field, round-trips", () => {
    const ops = nodeSetOps(orderNode(), { concept: { ...orderNode().concept, title: "Sales Order" } });
    expect(ops).toEqual([{ op: "node.set", slug: "order", title: "Sales Order" }]);
    expect(apply(BASE, ops).nodes.find((n) => n.key === "order")!.concept.title).toBe("Sales Order");
  });

  it("no-op edit (same value) emits []", () => {
    expect(nodeSetOps(orderNode(), { concept: orderNode().concept, position: { x: 9, y: 9 } })).toEqual([]);
  });

  it("add node → node.new, round-trips", () => {
    const ops = nodeNewOps({ slug: "invoice", title: "Invoice" });
    expect(ops).toEqual([{ op: "node.new", slug: "invoice", ty: "uml.Class", title: "Invoice" }]);
    expect(apply(BASE, ops).nodes.some((n) => n.key === "invoice")).toBe(true);
  });

  it("remove node → node.rm cascade, round-trips (its edges go too)", () => {
    const ops = nodeRmOps("customer");
    expect(ops).toEqual([{ op: "node.rm", slug: "customer", cascade: true }]);
    const g = apply(BASE, ops);
    expect(g.nodes.some((n) => n.key === "customer")).toBe(false);
    expect(g.edges.some((e) => e.to === "customer")).toBe(false);
  });

  it("rename → node.rename, round-trips", () => {
    const ops = nodeRenameOps("customer", "client");
    expect(ops).toEqual([{ op: "node.rename", from: "customer", to: "client" }]);
    expect(apply(BASE, ops).nodes.some((n) => n.key === "client")).toBe(true);
  });

  it("rename to the same slug emits []", () => {
    expect(nodeRenameOps("customer", "customer")).toEqual([]);
  });

  it("title/description writes route through the concept (the single stored source)", () => {
    // The patch carries the intended `concept`; nodeSetOps compares its title/desc
    // against the node's stored concept and emits a node.set for only what changed.
    const prev = { ...orderNode(), concept: { ...orderNode().concept, description: "concept-desc" } };
    // Write intent equals the stored concept value → no change → no op.
    expect(nodeSetOps(prev, { concept: { ...prev.concept, description: "concept-desc" } })).toEqual([]);
    // Write intent differs from the stored concept value → a single desc op.
    expect(nodeSetOps(prev, { concept: { ...prev.concept, description: "changed" } })).toEqual([
      { op: "node.set", slug: "order", desc: "changed" },
    ]);
    // Clearing a non-empty description to "" emits a single desc:"" op (not skipped).
    expect(nodeSetOps(prev, { concept: { ...prev.concept, description: "" } })).toEqual([
      { op: "node.set", slug: "order", desc: "" },
    ]);
  });
});

describe("attribute array diff", () => {
  const attrs = () => orderNode().attributes;

  it("addition → attr.add, round-trips", () => {
    const next: Attribute[] = [...attrs(), { name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" }];
    const ops = attrDiffOps("order", attrs(), next);
    expect(ops).toEqual([{ op: "attr.add", node: "order", name: "placedAt", ty: "Timestamp" }]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "order")!;
    expect(n.attributes.some((a) => a.name === "placedAt")).toBe(true);
  });

  it("removal → attr.rm, round-trips", () => {
    const next = attrs().filter((a) => a.name !== "total");
    const ops = attrDiffOps("order", attrs(), next);
    expect(ops).toEqual([{ op: "attr.rm", node: "order", name: "total" }]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "order")!;
    expect(n.attributes.some((a) => a.name === "total")).toBe(false);
  });

  it("changed field on a kept attribute → attr.set with only that field, round-trips", () => {
    const next = attrs().map((a) => (a.name === "total" ? { ...a, type: { name: "Cash" } } : a));
    const ops = attrDiffOps("order", attrs(), next);
    expect(ops).toEqual([{ op: "attr.set", node: "order", name: "total", ty: "Cash" }]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "order")!;
    expect(n.attributes.find((a) => a.name === "total")!.type.name).toBe("Cash");
  });

  it("rename (paired leftover) → attr.set with rename, round-trips", () => {
    const next = attrs().map((a) => (a.name === "id" ? { ...a, name: "orderId" } : a));
    const ops = attrDiffOps("order", attrs(), next);
    expect(ops).toEqual([
      { op: "attr.set", node: "order", name: "id", rename: "orderId", ty: "OrderId", mult: "1" },
    ]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "order")!;
    expect(n.attributes.some((a) => a.name === "orderId")).toBe(true);
    expect(n.attributes.some((a) => a.name === "id")).toBe(false);
  });

  it("no change emits []", () => {
    expect(attrDiffOps("order", attrs(), attrs())).toEqual([]);
  });
});

describe("value array diff", () => {
  const vals = () => statusNode().values ?? [];

  it("addition → value.add, round-trips", () => {
    const ops = valueDiffOps("status", vals(), [...vals(), "SHIPPED"]);
    expect(ops).toEqual([{ op: "value.add", node: "status", literal: "SHIPPED" }]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "status")!;
    expect(n.values).toContain("SHIPPED");
  });

  it("removal → value.rm, round-trips", () => {
    const ops = valueDiffOps("status", vals(), ["DRAFT"]);
    expect(ops).toEqual([{ op: "value.rm", node: "status", literal: "PLACED" }]);
    const n = apply(BASE, ops).nodes.find((x) => x.key === "status")!;
    expect(n.values).not.toContain("PLACED");
  });
});

describe("edge ops", () => {
  it("add edge → rel.add (associates, default ends), round-trips", () => {
    const ops = edgeAddOps("customer", "status");
    expect(ops).toEqual([{ op: "rel.add", source: "customer", kind: "associates", target: "status", ends: "1 to 1" }]);
    const g = apply(BASE, ops);
    expect(g.edges.some((e) => e.from === "customer" && e.to === "status")).toBe(true);
  });

  it("edit ends → rel.set, round-trips", () => {
    const ops = edgeSetOps(oc(), { toEnd: { multiplicity: "1..*", role: "customers" } });
    expect(ops).toEqual([
      { op: "rel.set", source: "order", kind: "associates", target: "customer", ends: "1 to 1..* customers" },
    ]);
    const e = apply(BASE, ops).edges.find((x) => x.from === "order" && x.to === "customer")!;
    expect(e.toEnd.multiplicity).toBe("1..*");
    expect(e.toEnd.role).toBe("customers");
  });

  it("change kind → rel.rm + rel.add, round-trips", () => {
    const ops = edgeSetOps(oc(), { kind: "aggregates" });
    expect(ops).toEqual([
      { op: "rel.rm", source: "order", kind: "associates", target: "customer" },
      { op: "rel.add", source: "order", kind: "aggregates", target: "customer", ends: "1 to 1" },
    ]);
    const e = apply(BASE, ops).edges.find((x) => x.from === "order" && x.to === "customer")!;
    expect(e.kind).toBe("aggregates");
  });

  it("remove edge → rel.rm, round-trips", () => {
    const ops = edgeRmOps(oc());
    expect(ops).toEqual([{ op: "rel.rm", source: "order", kind: "associates", target: "customer" }]);
    const g = apply(BASE, ops);
    expect(g.edges.some((x) => x.from === "order" && x.to === "customer")).toBe(false);
  });

  it("handles-only change emits [] (overlay keeps it)", () => {
    expect(edgeSetOps(oc(), { sourceHandle: "right", targetHandle: "left" })).toEqual([]);
  });
});

describe("updateNodeOps composite", () => {
  it("position-only patch emits []", () => {
    expect(updateNodeOps(orderNode(), { position: { x: 12, y: 34 } })).toEqual([]);
  });

  it("combines scalar + attribute changes", () => {
    const next = [...orderNode().attributes, { name: "note", type: { name: "String" }, multiplicity: "1" }];
    const ops = updateNodeOps(orderNode(), { concept: { ...orderNode().concept, title: "PO" }, attributes: next });
    expect(ops).toEqual([
      { op: "node.set", slug: "order", title: "PO" },
      { op: "attr.add", node: "order", name: "note", ty: "String" },
    ]);
  });
});
