import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import type { FlowDoc, ModelGraph } from "@waml/okf";
import FlowView from "./FlowView.svelte";

const B = "m/lifecycle";
const DOC: FlowDoc = {
  key: B,
  title: "Order Lifecycle",
  flavor: "stateMachine",
  nodes: [`${B}#initial`, `${B}#Placed`, `${B}#final`],
  edges: [`${B}#e0`, `${B}#e1`],
};
const GRAPH = {
  activityNodes: [
    { key: `${B}#initial`, id: "initial", behavior: B, kind: "initial" },
    { key: `${B}#Placed`, id: "Placed", behavior: B, kind: "plain", entry: "reserveStock" },
    { key: `${B}#final`, id: "final", behavior: B, kind: "final" },
  ],
  flowEdges: [
    { key: `${B}#e0`, kind: "controlFlow", behavior: B, from: `${B}#initial`, to: `${B}#Placed` },
    { key: `${B}#e1`, kind: "controlFlow", behavior: B, from: `${B}#Placed`, to: `${B}#final`, trigger: "deliver" },
  ],
} as unknown as ModelGraph;

describe("FlowView", () => {
  it("renders every flow node with its internals", () => {
    const { getByText } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
    expect(getByText("Placed")).toBeTruthy();
    expect(getByText("entry / reserveStock")).toBeTruthy();
  });

  it("gives every node a source and target handle so edges survive", () => {
    // SvelteFlow drops an edge unless the source node has a source handle and
    // the target node a target handle. jsdom never lays the graph out, so we
    // assert the invariant that was missing and caused every flow edge to vanish.
    const { container } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
    expect(container.querySelectorAll(".svelte-flow__handle.source").length).toBe(DOC.nodes.length);
    expect(container.querySelectorAll(".svelte-flow__handle.target").length).toBe(DOC.nodes.length);
  });
});
