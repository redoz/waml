import { describe, expect, it } from "vitest";
import type { ActivityNode, FlowDoc, FlowEdge, FlowFlavor, ModelGraph } from "@waml/okf";
import { flowToRf, resolveFlow, transitionLabel } from "./flowGraph";

const B = "m/lifecycle";
const k = (id: string) => `${B}#${id}`;
const nodes: ActivityNode[] = [
  { key: k("initial"), id: "initial", behavior: B, kind: "initial" },
  { key: k("Draft"), id: "Draft", behavior: B, kind: "plain" },
  { key: k("Ready to ship?"), id: "Ready to ship?", behavior: B, kind: "decision" },
  { key: k("final"), id: "final", behavior: B, kind: "final" },
];
const edges: FlowEdge[] = [
  { key: k("e0"), kind: "controlFlow", behavior: B, from: k("initial"), to: k("Draft") },
  { key: k("e1"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Ready to ship?"), trigger: "place", guard: "items > 0", effect: "reserve" },
  { key: k("e2"), kind: "controlFlow", behavior: B, from: k("Ready to ship?"), to: k("final"), else: true },
  { key: k("e3"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Missing") }, // unresolved target: not drawn, never errors
];
const view = { flavor: "stateMachine" as FlowFlavor, nodes, edges };

describe("transitionLabel", () => {
  it("renders UML 'trigger [guard] / effect' labels", () => {
    expect(transitionLabel(edges[1])).toBe("place [items > 0] / reserve");
    expect(transitionLabel(edges[2])).toBe("[else]");
    expect(transitionLabel(edges[0])).toBe("");
  });
});

describe("resolveFlow", () => {
  it("dereferences a view's node/edge keys against the model pools", () => {
    const graph = { activityNodes: nodes, flowEdges: edges } as unknown as ModelGraph;
    const doc: FlowDoc = { key: B, title: "T", flavor: "stateMachine", nodes: nodes.map((n) => n.key), edges: edges.map((e) => e.key) };
    const r = resolveFlow(doc, graph);
    expect(r.flavor).toBe("stateMachine");
    expect(r.nodes.map((n) => n.id)).toEqual(["initial", "Draft", "Ready to ship?", "final"]);
    expect(r.edges).toHaveLength(4);
  });
});

describe("flowToRf", () => {
  it("lays out every node and maps kinds to component types", () => {
    const { nodes: rf, edges: rfEdges } = flowToRf(view);
    expect(rf).toHaveLength(4);
    expect(rf.map((n) => n.type)).toEqual(["flowControl", "flowStep", "flowControl", "flowControl"]);
    // React node ids are pool keys; dagre TB puts initial above final.
    const y = (key: string) => rf.find((n) => n.id === key)!.position.y;
    expect(y(k("initial"))).toBeLessThan(y(k("final")));
    // the edge to a missing node is dropped, the rest are transitions
    expect(rfEdges).toHaveLength(3);
    expect(rfEdges.every((e) => e.type === "transition")).toBe(true);
  });

  it("carries the flavor and the source node's kind on each edge", () => {
    const { edges: rfEdges } = flowToRf(view);
    const data = (i: number) => rfEdges[i].data as { flavor: string; fromKind: string };
    expect(rfEdges.every((e) => (e.data as { flavor: string }).flavor === "stateMachine")).toBe(true);
    expect(data(0).fromKind).toBe("initial");
    expect(data(2).fromKind).toBe("decision");
  });
});
