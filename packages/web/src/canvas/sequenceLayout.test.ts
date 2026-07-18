import { describe, expect, it } from "vitest";
import type { SequenceDoc } from "@waml/okf";
import { layoutSequence } from "./sequenceLayout";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  nodes: [
    { node: "lifeline", id: "Customer", title: "Customer", ref: "s/customer" },
    { node: "lifeline", id: "order", title: "Order", alias: "order", ref: "s/order" },
    { node: "lifeline", id: "wh", title: "Warehouse", alias: "wh" },
    { node: "operand", id: "f0.o0", guard: "paid", items: [{ item: "message", edge: "m1" }] },
    { node: "operand", id: "f0.o1", items: [{ item: "message", edge: "m2" }] },
    { node: "fragment", id: "f0", kind: "alt", operands: ["f0.o0", "f0.o1"] },
  ],
  edges: [
    { id: "m0", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    { id: "m1", from: "order", verb: "calls", to: "wh", signature: "ship()" },
    { id: "m2", from: "order", verb: "sends", to: "Customer", signature: "paymentFailed()" },
    { id: "m3", from: "order", verb: "replies", to: "Customer" },
  ],
  items: [
    { item: "message", edge: "m0" },
    { item: "fragment", node: "f0" },
    { item: "message", edge: "m3" },
  ],
};

describe("layoutSequence", () => {
  it("places lifelines in declaration order and rows in document order", () => {
    const l = layoutSequence(DOC);
    expect(l.lifelines.map((x) => x.handle)).toEqual(["Customer", "order", "wh"]);
    expect(l.lifelines[0].x).toBeLessThan(l.lifelines[1].x);
    expect(l.lifelines[1].x).toBeLessThan(l.lifelines[2].x);

    const kinds = l.rows.map((r) => r.kind);
    expect(kinds[0]).toBe("message");
    expect(kinds[1]).toBe("fragmentStart");
    expect(kinds).toContain("operandDivider");
    expect(kinds[kinds.length - 2]).toBe("fragmentEnd");
    expect(kinds[kinds.length - 1]).toBe("message");

    // rows strictly increase in y (document order is time order)
    for (let i = 1; i < l.rows.length; i++) expect(l.rows[i].y).toBeGreaterThan(l.rows[i - 1].y);
  });

  it("resolves message endpoints to lifeline x positions by handle", () => {
    const l = layoutSequence(DOC);
    const first = l.rows.find((r) => r.kind === "message")! as Extract<(typeof l.rows)[number], { kind: "message" }>;
    const customerX = l.lifelines.find((x) => x.handle === "Customer")!.x;
    const orderX = l.lifelines.find((x) => x.handle === "order")!.x;
    expect(first.fromX).toBe(customerX);
    expect(first.toX).toBe(orderX);
    expect(first.self).toBe(false);
  });

  it("marks a message with equal endpoints as a self message", () => {
    const selfDoc: SequenceDoc = {
      ...DOC,
      edges: [{ id: "m0", from: "order", verb: "calls", to: "order", signature: "validate()" }],
      items: [{ item: "message", edge: "m0" }],
    };
    const l = layoutSequence(selfDoc);
    const row = l.rows[0] as Extract<(typeof l.rows)[number], { kind: "message" }>;
    expect(row.self).toBe(true);
  });
});
