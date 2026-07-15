import { describe, expect, it } from "vitest";
import type { SequenceDoc } from "@waml/okf";
import { layoutSequence } from "./sequenceLayout";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  lifelines: [
    { title: "Customer", ref: "s/customer" },
    { title: "Order", alias: "order", ref: "s/order" },
    { title: "Warehouse", alias: "wh" },
  ],
  messages: [
    { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    {
      item: "fragment",
      kind: "alt",
      operands: [
        { guard: "paid", items: [{ item: "message", from: "order", verb: "calls", to: "wh", signature: "ship()" }] },
        { items: [{ item: "message", from: "order", verb: "sends", to: "Customer", signature: "paymentFailed()" }] },
      ],
    },
    { item: "message", from: "order", verb: "replies", to: "Customer" },
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
      messages: [{ item: "message", from: "order", verb: "calls", to: "order", signature: "validate()" }],
    };
    const l = layoutSequence(selfDoc);
    const row = l.rows[0] as Extract<(typeof l.rows)[number], { kind: "message" }>;
    expect(row.self).toBe(true);
  });
});
