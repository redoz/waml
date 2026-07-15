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

  it("marks a `destroys` message with its own ✕ marker, distinct from `sends`", () => {
    const doc: SequenceDoc = {
      key: "s/teardown",
      title: "Teardown",
      lifelines: [{ title: "Manager" }, { title: "Worker" }],
      messages: [
        { item: "message", from: "Manager", verb: "sends", to: "Worker", signature: "stop()" },
        { item: "message", from: "Manager", verb: "destroys", to: "Worker" },
      ],
    };
    const { container } = render(SequenceView, { props: { doc } });
    // The destroys-marker definition exists exactly once …
    expect(container.querySelectorAll("marker#seq-arrow-x").length).toBe(1);
    // … and only the `destroys` row's line/path references it — the `sends` row
    // keeps the default open-arrow marker.
    const lines = [...container.querySelectorAll("line, path")];
    const withXMarker = lines.filter((el) => el.getAttribute("marker-end") === "url(#seq-arrow-x)");
    const withOpenMarker = lines.filter((el) => el.getAttribute("marker-end") === "url(#seq-arrow-open)");
    expect(withXMarker.length).toBe(1);
    expect(withOpenMarker.length).toBe(1);
  });
});
