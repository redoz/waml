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

  it("gives every node a source and target handle so edges survive", () => {
    // SvelteFlow's getEdgePosition() returns null — dropping the edge before it
    // ever renders — unless the source node has a source handle and the target
    // node a target handle (isNodeInitialized in @xyflow/system). jsdom never
    // lays the graph out, so we can't assert the drawn edges here; instead we
    // assert the invariant that was missing and caused every flow edge to vanish.
    const { container } = render(FlowView, { props: { doc: DOC } });
    expect(container.querySelectorAll(".svelte-flow__handle.source").length).toBe(DOC.nodes.length);
    expect(container.querySelectorAll(".svelte-flow__handle.target").length).toBe(DOC.nodes.length);
  });
});
