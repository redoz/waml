import { useCallback, useMemo, useSyncExternalStore, useState } from "react";
import type { FC } from "react";
import {
  ReactFlow as ReactFlowBase,
  Background,
  BackgroundVariant,
  Controls,
  type Node,
  type Edge,
  type NodeChange,
  type EdgeChange,
  type Connection,
  type ReactFlowProps,
  useReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./canvas.css";

import dagre from "@dagrejs/dagre";

import { createModelStore } from "../../state/model";
import type { ModelNode, ModelEdge } from "@mc/okf";

import { TopBar } from "../TopBar";
import { Dock, type Tool } from "./Dock";
import { MartNode } from "./MartNode";
import { RelEdge } from "./RelEdge";
import { Inspector } from "../inspector/Inspector";

// Cast to FC to avoid generic component JSX typing issues with @types/react 18.3
const ReactFlow = ReactFlowBase as unknown as FC<ReactFlowProps>;

// ── store singleton ──────────────────────────────────────────────────────────
const store = createModelStore();

// ── helpers to convert between model and RF types ───────────────────────────
function toRFNode(n: ModelNode): Node {
  return {
    id: n.key,
    type: "mart",
    position: n.position,
    data: { ...n } as unknown as Record<string, unknown>,
  };
}

function toRFEdge(e: ModelEdge): Edge {
  return {
    id: e.id,
    source: e.from,
    target: e.to,
    type: "rel",
    data: { keys: e.keys, bidirectional: e.bidirectional } as unknown as Record<string, unknown>,
  };
}

// ── Dagre auto-layout ────────────────────────────────────────────────────────
const NODE_W = 200;
const NODE_H = 90;

function runDagreLayout(nodes: ModelNode[], edges: ModelEdge[]): Map<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 60, ranksep: 150 });
  nodes.forEach(n => g.setNode(n.key, { width: NODE_W, height: NODE_H }));
  edges.forEach(e => g.setEdge(e.from, e.to));
  dagre.layout(g);
  const positions = new Map<string, { x: number; y: number }>();
  nodes.forEach(n => {
    const pos = g.node(n.key);
    positions.set(n.key, { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 });
  });
  return positions;
}

// ── Selection types ──────────────────────────────────────────────────────────
type Selection =
  | { type: "node"; id: string }
  | { type: "edge"; id: string }
  | null;

// ── Inner canvas (needs ReactFlowProvider context) ────────────────────────────
const nodeTypes = { mart: MartNode };
const edgeTypes = { rel: RelEdge };

function CanvasInner() {
  const graph = useSyncExternalStore(store.subscribe, store.get);
  const { screenToFlowPosition } = useReactFlow();

  const [selection, setSelection] = useState<Selection>(null);
  const [tool, setTool] = useState<Tool>("select");

  // Convert model → RF nodes/edges
  const rfNodes = useMemo(() => graph.nodes.map(toRFNode), [graph.nodes]);
  const rfEdges = useMemo(() => graph.edges.map(toRFEdge), [graph.edges]);

  // ── Node changes (drag / select / remove) ──────────────────────────────────
  const onNodesChange = useCallback((changes: NodeChange[]) => {
    // Persist position only at drag end (dragging === false) to avoid a store
    // write — and a re-render + global-listener churn — on every drag-move tick.
    changes.forEach(change => {
      if (change.type === "position" && change.position && !change.dragging) {
        store.updateNode(change.id, { position: change.position });
      }
    });
  }, []);

  // ── Edge changes (no-op; store is source of truth) ────────────────────────
  const onEdgesChange = useCallback((_changes: EdgeChange[]) => {
    // intentionally empty
  }, []);

  // ── Connect handler ────────────────────────────────────────────────────────
  const onConnect = useCallback((connection: Connection) => {
    if (!connection.source || !connection.target) return;
    store.addEdge(connection.source, connection.target);
  }, []);

  // ── Pane click → deselect ─────────────────────────────────────────────────
  const onPaneClick = useCallback(() => {
    setSelection(null);
  }, []);

  // ── Node click → select ────────────────────────────────────────────────────
  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelection({ type: "node", id: node.id });
  }, []);

  // ── Edge click → select ────────────────────────────────────────────────────
  const onEdgeClick = useCallback((_: React.MouseEvent, edge: Edge) => {
    setSelection({ type: "edge", id: edge.id });
  }, []);

  // ── Auto-layout + tool handler ─────────────────────────────────────────────
  // Read the graph from the store at call time so this stays stable and doesn't
  // re-create (and churn the Dock keydown listener) on every drag-move tick.
  const handleToolChange = useCallback((t: Tool) => {
    if (t === "layout") {
      const { nodes, edges } = store.get();
      const positions = runDagreLayout(nodes, edges);
      positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
      return;
    }
    setTool(t);
  }, []);

  // ── Keyboard delete ────────────────────────────────────────────────────────
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if ((e.key === "Delete" || e.key === "Backspace") && selection) {
      const tag = (e.target as HTMLElement).tagName;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
      if (selection.type === "node") store.removeNode(selection.id);
      else store.removeEdge(selection.id);
      setSelection(null);
    }
  }, [selection]);

  // ── Double-click on empty pane → add node (works in any tool, like the prototype) ──
  const handleWrapperDoubleClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    // Only fire when clicking the pane (not on a node card or edge)
    const target = e.target as HTMLElement;
    if (target.closest(".react-flow__node") || target.closest(".react-flow__edge")) return;
    const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const n = store.addNode({ x: position.x - NODE_W / 2, y: position.y - NODE_H / 2 });
    setSelection({ type: "node", id: n.key });
    setTool("select");
  }, [screenToFlowPosition]);

  // ── Pending count for TopBar ───────────────────────────────────────────────
  const pendingCount = graph.nodes.filter(n => n.status === "pending").length;

  // ── Canvas class based on tool ─────────────────────────────────────────────
  const canvasClass =
    tool === "add" ? "canvas-add" :
    tool === "connect" ? "canvas-connect" : "";

  return (
    <div
      className="flex flex-col h-screen overflow-hidden bg-[#f7f8fa]"
      style={{ fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif" }}
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      <TopBar pendingCount={pendingCount} />

      <div className="flex flex-1 min-h-0 relative">
        {/* Left tool dock */}
        <Dock activeTool={tool} onToolChange={handleToolChange} />

        {/* React Flow canvas */}
        <div
          className={`flex-1 relative ${canvasClass}`}
          onDoubleClick={handleWrapperDoubleClick}
        >
          <ReactFlow
            nodes={rfNodes}
            edges={rfEdges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onPaneClick={onPaneClick}
            onNodeClick={onNodeClick}
            onEdgeClick={onEdgeClick}
            fitView={false}
            minZoom={0.4}
            maxZoom={1.6}
            nodesDraggable={tool === "select"}
            nodesConnectable={true}
            selectNodesOnDrag={false}
            panOnDrag={tool === "select"}
            zoomOnScroll={true}
            deleteKeyCode={null}
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} color="#e2e6ec" />
            <Controls />
          </ReactFlow>

          {/* Empty canvas CTA */}
          {graph.nodes.length === 0 && (
            <div
              className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-center text-slate-500 pointer-events-none z-[1]"
              style={{ fontSize: 15 }}
            >
              <div><strong className="text-slate-900">Empty canvas</strong></div>
              <div className="mt-[6px] text-[13px] leading-[1.6]">
                Double-click anywhere to add an object.<br />
                Drag from a node's port to create a relationship.
              </div>
            </div>
          )}
        </div>

        {/* Right inspector drawer */}
        <Inspector
          selection={selection}
          nodes={graph.nodes}
          edges={graph.edges}
          onUpdateNode={store.updateNode}
          onUpdateEdge={store.updateEdge}
          onClose={() => setSelection(null)}
        />
      </div>
    </div>
  );
}

// ── Public export ─────────────────────────────────────────────────────────────
export function CanvasApp() {
  return (
    <ReactFlowProvider>
      <CanvasInner />
    </ReactFlowProvider>
  );
}
