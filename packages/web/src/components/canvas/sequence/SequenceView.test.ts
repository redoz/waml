import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import type { SequenceDoc } from "@waml/okf";
import SequenceView from "./SequenceView.svelte";

const DOC: SequenceDoc = {
  key: "s/place-order",
  title: "Place Order",
  lifelines: [{ title: "Customer" }, { title: "Order", alias: "order" }],
  messages: [
    { item: "message", from: "Customer", verb: "calls", to: "order", signature: "place(items)" },
    { item: "message", from: "order", verb: "replies", to: "Customer", signature: "confirmation" },
  ],
};

describe("SequenceView", () => {
  it("renders every lifeline handle and message signature", () => {
    const { getByText } = render(SequenceView, { props: { doc: DOC } });
    expect(getByText("Customer")).toBeTruthy();
    expect(getByText("order")).toBeTruthy();
    expect(getByText("place(items)")).toBeTruthy();
    expect(getByText("confirmation")).toBeTruthy();
  });
});
