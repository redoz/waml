import { describe, expect, it } from "vitest";
import type { FlowDoc } from "@waml/okf";
import { flowToRf, transitionLabel } from "./flowGraph";

const DOC: FlowDoc = {
  key: "m/lifecycle",
  title: "Order Lifecycle",
  flavor: "stateMachine",
  nodes: [
    { id: "initial", kind: "initial" },
    { id: "Draft", kind: "plain" },
    { id: "Ready to ship?", kind: "decision" },
    { id: "final", kind: "final" },
  ],
  edges: [
    { from: "initial", to: "Draft" },
    { from: "Draft", to: "Ready to ship?", trigger: "place", guard: "items > 0", effect: "reserve" },
    { from: "Ready to ship?", to: "final", else: true },
    { from: "Draft", to: "Missing" }, // unresolved target: not drawn, never errors
  ],
};

describe("transitionLabel", () => {
  it("renders UML 'trigger [guard] / effect' labels", () => {
    expect(transitionLabel(DOC.edges[1])).toBe("place [items > 0] / reserve");
    expect(transitionLabel(DOC.edges[2])).toBe("[else]");
    expect(transitionLabel(DOC.edges[0])).toBe("");
  });
});

describe("flowToRf", () => {
  it("lays out every node and maps kinds to component types", () => {
    const { nodes, edges } = flowToRf(DOC);
    expect(nodes).toHaveLength(4);
    expect(nodes.map((n) => n.type)).toEqual(["flowControl", "flowStep", "flowControl", "flowControl"]);
    // dagre TB: the initial node sits above the final node
    const y = (id: string) => nodes.find((n) => n.id === id)!.position.y;
    expect(y("initial")).toBeLessThan(y("final"));
    // the edge to a missing node is dropped, the rest are transitions
    expect(edges).toHaveLength(3);
    expect(edges.every((e) => e.type === "transition")).toBe(true);
  });
});
