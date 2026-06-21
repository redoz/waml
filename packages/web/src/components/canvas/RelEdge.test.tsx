import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { ReactFlow, ReactFlowProvider } from "@xyflow/react";
import { RelEdge } from "./RelEdge";

const edgeTypes = { rel: RelEdge };

// `EdgeLabelRenderer` portals into a DOM node that the real `<ReactFlow>`
// renderer creates on mount (it does not exist with `<ReactFlowProvider>`
// alone), and the edge path itself is only computed once each node's handle
// bounds are known. jsdom has no real layout/ResizeObserver, so handle
// measurement never happens automatically; providing static `handles` on
// each node (matching the `NodeBase["handles"]` fallback @xyflow/react reads
// when `internals.handleBounds` is unset) satisfies `isNodeInitialized`
// without depending on layout. This is purely test scaffolding — it renders
// the same `RelEdge` component the app uses, via the real edge type wiring.
const handles = [
  { id: null, type: "source" as const, position: "bottom" as const, x: 0, y: 0, width: 1, height: 1 },
  { id: null, type: "target" as const, position: "top" as const, x: 0, y: 0, width: 1, height: 1 },
];

const nodes = [
  { id: "a", position: { x: 0, y: 0 }, data: {}, width: 100, height: 50, measured: { width: 100, height: 50 }, handles },
  { id: "b", position: { x: 100, y: 0 }, data: {}, width: 100, height: 50, measured: { width: 100, height: 50 }, handles },
];

function renderEdge(data: any) {
  return render(
    <ReactFlowProvider>
      <div style={{ width: 400, height: 400 }}>
        <ReactFlow
          nodes={nodes as any}
          edges={[{ id: "e1", source: "a", target: "b", type: "rel", data }]}
          edgeTypes={edgeTypes}
        />
      </div>
    </ReactFlowProvider>,
  );
}

describe("RelEdge cardinality pill", () => {
  it("shows the pill when cardinality is set", () => {
    const { queryByText } = renderEdge({ keys: [{ left: "x", right: "y" }], bidirectional: false, cardinality: "N:1" });
    expect(queryByText("N:1")).toBeTruthy();
  });
  it("shows no pill when cardinality is unset", () => {
    const { queryByText } = renderEdge({ keys: [{ left: "x", right: "y" }], bidirectional: false });
    expect(queryByText("N:1")).toBeNull();
  });
});
