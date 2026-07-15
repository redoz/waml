import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import type { FlowDoc } from "@waml/okf";
import FlowView from "./FlowView.svelte";

const DOC: FlowDoc = {
  key: "m/lifecycle",
  title: "Order Lifecycle",
  flavor: "stateMachine",
  nodes: [
    { id: "initial", kind: "initial" },
    { id: "Placed", kind: "plain", entry: "reserveStock" },
    { id: "final", kind: "final" },
  ],
  edges: [
    { from: "initial", to: "Placed" },
    { from: "Placed", to: "final", trigger: "deliver" },
  ],
};

describe("FlowView", () => {
  it("renders every flow node with its internals", () => {
    const { getByText } = render(FlowView, { props: { doc: DOC } });
    expect(getByText("Placed")).toBeTruthy();
    expect(getByText("entry / reserveStock")).toBeTruthy();
  });
});
