import { useCallback, useEffect, useSyncExternalStore, useState } from "react";
import type { FC } from "react";
import {
  ReactFlow as ReactFlowBase,
  Background,
  BackgroundVariant,
  Controls,
  ConnectionMode,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type NodeChange,
  type Connection,
  type ReactFlowProps,
  useReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./canvas.css";

import dagre from "@dagrejs/dagre";
import { MessageSquare } from "lucide-react";

import { createModelStore } from "../../state/model";
import { loadPersistedGraph, persistGraph } from "../../state/persist";
import { loadViewMode, persistViewMode, type ViewMode } from "../../state/viewMode";
import { loadRelLabelMode, persistRelLabelMode, type RelLabelMode } from "../../state/relLabels";
import { loadModelName, persistModelName, DEFAULT_MODEL_NAME, templateModelName } from "../../state/modelName";
import type { ModelNode, ModelEdge, ModelGraph } from "@mc/okf";

import { graphToBundleFiles, downloadBundle } from "../../okf/io";
import { buildShareUrl, readSharedModel, readSharedName, clearSharedModelFromUrl } from "../../share/url";
import { readTemplateModel, clearTemplateFromUrl } from "../../lib/templateLink";
import { exportCanvasSvg } from "../../share/exportImage";

import { TopBar } from "../TopBar";
import { ImportDialog } from "../ImportDialog";
import { mergeGraphs } from "../../sync/merge";
import { LibraryDialog } from "../LibraryDialog";
import { TemplateApplyDialog } from "../TemplateApplyDialog";
import { WelcomeDialog } from "../WelcomeDialog";
import { ClearCanvasDialog } from "../ClearCanvasDialog";
import { Dock, type Tool } from "./Dock";
import { OkfNode } from "./nodes/OkfNode";
import { RelEdge } from "./RelEdge";
import { AnchorEdge } from "./AnchorEdge";
import { buildRfEdges, buildAnchorEdges, isEdgeReconnectable } from "./edges";
import { erdAwareNodeSize } from "./layoutSize";
import { Inspector } from "../inspector/Inspector";
import { getProfile } from "../../profiles";
import { RightRail } from "../rail/RightRail";
import { ModelSheet } from "../rail/ModelSheet";
import { useRightPanel } from "../rail/useRightPanel";
import { SharePanel } from "../rail/SharePanel";
import { GoalDialog } from "../GoalDialog";
import { loadGoal, persistGoal, type BusinessGoal } from "../../state/goal";

// Cast to FC to avoid generic component JSX typing issues with @types/react 18.3
const ReactFlow = ReactFlowBase as unknown as FC<ReactFlowProps>;

// ── store singleton (exported so external modules can share this instance) ───
// Precedence: a `?template=<id>` deep-link and a `#m=…` share link are both
// explicit "open this model" intents, so they win over localStorage; otherwise
// rehydrate from localStorage so a refresh doesn't wipe work.
//
// `?template=<id>` opens a named built-in template (the CTA target for the blog
// gallery, launch emails and posts). Templates ship at (0,0), so we Dagre-lay it
// out here — runDagreLayout is a hoisted function declaration, available now.
const templateGraph = readTemplateModel();
clearTemplateFromUrl(); // strip the param (clean URL on refresh) even if the id was unknown
let templateInitial: ModelGraph | undefined;
if (templateGraph) {
  const positions = runDagreLayout(templateGraph.nodes, templateGraph.edges, loadViewMode());
  templateInitial = { ...templateGraph, nodes: templateGraph.nodes.map(n => ({ ...n, position: positions.get(n.key) ?? n.position })) };
}

const sharedGraph = readSharedModel();
const sharedModelName = readSharedName(); // name carried alongside a shared link, if any
const persistedGraph = loadPersistedGraph();
export const store = createModelStore(templateInitial ?? sharedGraph ?? persistedGraph);
if (templateInitial || sharedGraph) {
  // Persist the opened model right away — it's the store's initial value, so it
  // never fires a change that the mirror-to-localStorage effect would catch; a
  // refresh would otherwise lose it once the URL is cleaned.
  persistGraph(store.get());
}
// Drop the share payload from the address bar so a refresh doesn't re-clobber the
// canvas and the URL stays clean (the template param is already cleared above).
if (sharedGraph) clearSharedModelFromUrl();

// A truly first-ever visit has no template deep-link, no persisted model and no
// shared link. Captured at module load — before the persist effect writes an
// (empty) graph — so it stays true for the session. Gates the first-screen
// "start" chooser: shown once for new visitors, never over an opened model.
const isFirstVisit = !templateInitial && !sharedGraph && persistedGraph === undefined;

// ── helpers to convert between model and RF types ───────────────────────────
function toRFNode(n: ModelNode, viewMode: ViewMode, profileName: string): Node {
  return { id: n.key, type: "okf", position: n.position, data: { ...n, _viewMode: viewMode, _profile: profileName } as unknown as Record<string, unknown> };
}

// ── Dagre auto-layout ────────────────────────────────────────────────────────
const NODE_W = 200;
const NODE_H = 90;

function runDagreLayout(nodes: ModelNode[], edges: ModelEdge[], viewMode: ViewMode): Map<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 60, ranksep: 150 });
  nodes.forEach(n => { const s = erdAwareNodeSize(n, viewMode); g.setNode(n.key, { width: s.width, height: s.height }); });
  edges.forEach(e => g.setEdge(e.from, e.to));
  dagre.layout(g);
  const positions = new Map<string, { x: number; y: number }>();
  nodes.forEach(n => {
    const pos = g.node(n.key);
    const s = erdAwareNodeSize(n, viewMode);
    positions.set(n.key, { x: pos.x - s.width / 2, y: pos.y - s.height / 2 });
  });
  return positions;
}

// ── Selection types ──────────────────────────────────────────────────────────
type Selection =
  | { type: "node"; id: string }
  | { type: "edge"; id: string }
  | null;

// Titles shown in the right Sheet header per active panel.
const SHEET_TITLES: Record<NonNullable<ReturnType<typeof useRightPanel>["active"]>, string> = {
  inspect: "Inspect", share: "Share model",
};

// ── Inner canvas (needs ReactFlowProvider context) ────────────────────────────
const nodeTypes = { okf: OkfNode };
const edgeTypes = { rel: RelEdge, anchor: AnchorEdge };

function CanvasInner() {
  const graph = useSyncExternalStore(store.subscribe, store.get);
  const { screenToFlowPosition, fitView } = useReactFlow();
  // True briefly during auto-layout so nodes glide (CSS transition) to their new
  // positions instead of snapping.
  const [layoutAnimating, setLayoutAnimating] = useState(false);

  const [selection, setSelection] = useState<Selection>(null);
  // Single right-side panel state (which rail entry is open in the Sheet).
  const panel = useRightPanel();
  // Selecting a node/edge auto-opens the Inspect panel — preserves current UX.
  useEffect(() => { if (selection) panel.open("inspect"); }, [selection]); // eslint-disable-line react-hooks/exhaustive-deps
  // Business goal — a stored objective ({niche, goal}) persisted in localStorage.
  // Standalone: captured via the Business Goal dialog and shown on the TopBar.
  const [goal, setGoalState] = useState<BusinessGoal | null>(loadGoal());
  const [showGoal, setShowGoal] = useState(false);
  const [tool, setTool] = useState<Tool>("select");
  const [viewMode, setViewMode] = useState<ViewMode>(loadViewMode());
  const [relLabelMode, setRelLabelMode] = useState<RelLabelMode>(loadRelLabelMode());
  const handleRelLabelModeChange = useCallback((mode: RelLabelMode) => {
    setRelLabelMode(mode);
    persistRelLabelMode(mode);
  }, []);
  const [showImport, setShowImport] = useState(false);
  const [showLibrary, setShowLibrary] = useState(false);
  // A template chosen from the library while the canvas already had content —
  // held until the user confirms Replace vs Merge in the TemplateApplyDialog.
  const [pendingTemplate, setPendingTemplate] = useState<{ graph: ModelGraph; name: string } | null>(null);
  // First-screen chooser — shown once to brand-new visitors (no persisted model).
  const [showWelcome, setShowWelcome] = useState(isFirstVisit);
  const [shareToast, setShareToast] = useState<string | null>(null);
  const [showClear, setShowClear] = useState(false);
  // Editable model name (shown in the top bar, persisted locally).
  // A shared link's name wins on first load (opening someone's named model);
  // otherwise restore the locally-persisted name.
  const [modelName, setModelName] = useState(sharedModelName ?? loadModelName());
  useEffect(() => { persistModelName(modelName); }, [modelName]);

  // React Flow owns the live node/edge arrays so dragging follows the cursor
  // smoothly (RF applies position changes frame-by-frame). The model store stays
  // the source of truth: we sync store → RF on structural/data changes, and write
  // positions back to the store only at drag end.
  const [rfNodes, setRfNodes, onRfNodesChange] = useNodesState<Node>([]);
  const [rfEdges, setRfEdges, onRfEdgesChange] = useEdgesState<Edge>([]);

  useEffect(() => {
    setRfNodes(graph.nodes.map(n => toRFNode(n, viewMode, "uml-domain")));
  }, [graph.nodes, viewMode, setRfNodes]);
  useEffect(() => {
    const emphasizeMultiplicity = getProfile("uml-domain").emphasize.includes("multiplicity");
    setRfEdges([
      ...buildRfEdges(graph.edges, graph.nodes, viewMode, relLabelMode, emphasizeMultiplicity),
      ...buildAnchorEdges(graph.nodes, graph.edges),
    ]);
  }, [graph.edges, graph.nodes, viewMode, relLabelMode, setRfEdges]);

  // Mark only the selected relationship as reconnectable so dragging an endpoint
  // moves the line the user picked (not whichever overlapping edge RF would grab),
  // and raise it above the others so its reconnect anchor isn't buried under an
  // overlapping line (otherwise the drag handle never appears). Patches in place —
  // never touches `selected` — and re-applies after any rebuild of the edges array.
  useEffect(() => {
    const selId = selection?.type === "edge" ? selection.id : null;
    setRfEdges(eds => eds.map(e => {
      const modelEdgeId = (e.data as { modelEdgeId?: string } | undefined)?.modelEdgeId;
      const reconnectable = isEdgeReconnectable(modelEdgeId, selId);
      const zIndex = modelEdgeId != null && modelEdgeId === selId ? 1000 : 0;
      return (e.reconnectable === reconnectable && e.zIndex === zIndex) ? e : { ...e, reconnectable, zIndex };
    }));
  }, [selection, graph.edges, graph.nodes, setRfEdges]);

  // Mirror the model to localStorage on every change so a refresh/crash doesn't
  // lose work.
  useEffect(() => { persistGraph(graph); }, [graph]);

  const onNodesChange = useCallback((changes: NodeChange[]) => {
    onRfNodesChange(changes);                       // animate the drag live
    for (const c of changes) {
      if (c.type === "position" && c.position && c.dragging === false) {
        store.updateNode(c.id, { position: c.position }); // persist final position
      }
    }
  }, [onRfNodesChange]);

  // ── Connect handler ────────────────────────────────────────────────────────
  // Drag an existing edge end onto another port/node to re-route it (for a tidy picture).
  const onReconnect = useCallback((oldEdge: Edge, conn: Connection) => {
    if (!conn.source || !conn.target || conn.source === conn.target) return;
    store.updateEdge(oldEdge.id, { from: conn.source, to: conn.target, sourceHandle: conn.sourceHandle, targetHandle: conn.targetHandle });
  }, []);

  const onConnect = useCallback((connection: Connection) => {
    if (!connection.source || !connection.target) return;
    // Open the new edge in the inspector right away so the user can set join
    // keys without an extra click to select the freshly-drawn line.
    const e = store.addEdge(connection.source, connection.target, connection.sourceHandle, connection.targetHandle);
    if (e) setSelection({ type: "edge", id: e.id });
  }, []);

  // ── Pane click → add (in Add tool) or deselect ────────────────────────────
  const onPaneClick = useCallback((e: React.MouseEvent) => {
    if (tool === "add") {
      const pos = screenToFlowPosition({ x: e.clientX, y: e.clientY });
      const n = store.addNode({ x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 });
      setSelection({ type: "node", id: n.key });
      setTool("select");
      return;
    }
    setSelection(null);
  }, [tool, screenToFlowPosition]);

  // ── Node click → select ────────────────────────────────────────────────────
  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    setSelection({ type: "node", id: node.id });
  }, []);

  // ── Edge click → select ────────────────────────────────────────────────────
  // ERD mode may render several RF edges per model edge (e.g. "e1::0"); strip
  // the suffix so the inspector still selects the underlying model edge.
  // Invariant: model edge ids are generated as "e<n>" and never contain "::",
  // so this split is a safe no-op in compact mode (plain ids pass through unchanged).
  const onEdgeClick = useCallback((_: React.MouseEvent, edge: Edge) => {
    setSelection({ type: "edge", id: edge.id.split("::")[0] });
  }, []);

  // ── Auto-layout + tool handler ─────────────────────────────────────────────
  // Read the graph from the store at call time so this stays stable and doesn't
  // re-create (and churn the Dock keydown listener) on every drag-move tick.
  const handleToolChange = useCallback((t: Tool) => {
    if (t === "layout") {
      const { nodes, edges } = store.get();
      const positions = runDagreLayout(nodes, edges, viewMode);
      // Turn on node transitions, move everything, then frame the result — so the
      // model visibly "organizes itself" instead of snapping. Cleared after the
      // glide so dragging stays instant.
      setLayoutAnimating(true);
      positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
      setTimeout(() => fitView({ duration: 500, padding: 0.18 }), 30);
      setTimeout(() => setLayoutAnimating(false), 560);
      return;
    }
    setTool(t);
  }, [viewMode, fitView]);

  const handleToggleView = useCallback(() => {
    setViewMode(prev => {
      const next = prev === "erd" ? "compact" : "erd";
      persistViewMode(next);
      return next;
    });
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
    if (target.closest("[data-dock]")) return; // double-clicking the toolbar shouldn't drop a node
    const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const n = store.addNode({ x: position.x - NODE_W / 2, y: position.y - NODE_H / 2 });
    setSelection({ type: "node", id: n.key });
    setTool("select");
  }, [screenToFlowPosition]);

  // ── Import / Export / Push handlers ───────────────────────────────────────
  const handleExport = useCallback(() => {
    const title = modelName.trim() || "model-okf";
    const files = graphToBundleFiles(store.get(), title);
    downloadBundle(files, title);
  }, [modelName]);

  // Clear the canvas: permanently wipe every node + edge (keep the selected
  // storage). No undo — the dialog warns and offers an OKF export first.
  const clearCanvas = useCallback(() => {
    store.set({ nodes: [], edges: [], diagrams: [] });
    setSelection(null);
    setShowClear(false);
    setModelName(DEFAULT_MODEL_NAME);
  }, []);

  const handleExportAndClear = useCallback(() => {
    handleExport();
    clearCanvas();
  }, [handleExport, clearCanvas]);

  // Export the canvas as an SVG (whole model, OWOX watermark). Uses the live RF
  // node list (measured sizes) to frame the export.
  const imageName = modelName.trim() || "model";
  const handleExportSvg = useCallback(() => {
    exportCanvasSvg(rfNodes, imageName).catch(() => setShareToast("Couldn't export the image — please try again."));
  }, [rfNodes, imageName]);

  // Copy a shareable link that reopens this exact model. Falls back to a prompt
  // if the clipboard API is blocked (insecure context / permissions).
  const handleShare = useCallback(async () => {
    const url = buildShareUrl(store.get(), modelName);
    // The whole model rides in the link's #hash, so it works on whatever origin
    // serves the app. On localhost that's only this machine — flag it so a local
    // dev doesn't think the link is broken; on model.owox.com it just works.
    const isLocal = /^(localhost|127\.|0\.0\.0\.0|\[::1\])/.test(location.hostname);
    const msg = isLocal
      ? "Link copied — note: a localhost link only opens on this machine. Deploy to share it."
      : "Link copied — anyone with it can open this model.";
    try {
      await navigator.clipboard.writeText(url);
      setShareToast(msg);
    } catch {
      window.prompt("Copy this shareable link:", url);
    }
  }, [modelName]);

  // Auto-layout a freshly loaded graph (import or template). The OKF format does
  // not persist node positions (Dagre re-lays out on load, by design), so without
  // this every imported node piles up at the origin and must be dragged apart.
  const withLayout = useCallback((g: ModelGraph): ModelGraph => {
    const positions = runDagreLayout(g.nodes, g.edges, viewMode);
    return { ...g, nodes: g.nodes.map(n => ({ ...n, position: positions.get(n.key) ?? n.position })) };
  }, [viewMode]);

  // Merge a freshly loaded graph into the canvas, laying out only the new nodes
  // so the existing layout isn't reshuffled. Shared by OKF import + templates (merge).
  const applyMergeWithLayout = useCallback((g: ModelGraph) => {
    const { graph, newKeys } = mergeGraphs(store.get(), g);
    const positions = runDagreLayout(graph.nodes, graph.edges, viewMode);
    store.set({ ...graph, nodes: graph.nodes.map(n => newKeys.has(n.key) ? { ...n, position: positions.get(n.key) ?? n.position } : n) });
  }, [viewMode]);

  const handleImportConfirm = useCallback((g: ModelGraph, mode: "replace" | "merge") => {
    if (mode === "merge") applyMergeWithLayout(g);
    else {
      const hasPositions = g.nodes.some(n => n.position.x !== 0 || n.position.y !== 0);
      store.set(hasPositions ? g : withLayout(g));
    }
    setShowImport(false);
  }, [withLayout, applyMergeWithLayout]);

  const applyTemplate = useCallback((g: ModelGraph, mode: "replace" | "merge") => {
    // Auto-layout the template (templates ship at 0,0).
    if (mode === "merge") applyMergeWithLayout(g);
    else store.set(withLayout(g));
  }, [withLayout, applyMergeWithLayout]);

  const handleUseTemplate = useCallback((g: ModelGraph, name: string) => {
    // Empty canvas → drop the template straight in. Non-empty → ask Replace vs
    // Merge first (mirrors the OKF/OWOX import dialogs) so existing work isn't
    // silently wiped.
    if (store.get().nodes.length === 0) {
      setModelName(templateModelName(name)); // "My {template} OKF with OWOX"
      applyTemplate(g, "replace");
      setShowLibrary(false);
    } else {
      setPendingTemplate({ graph: g, name });
    }
  }, [applyTemplate]);

  const handleTemplateApplyConfirm = useCallback((mode: "replace" | "merge") => {
    if (pendingTemplate) {
      // Replacing = a fresh model from this template, so re-seed the name; merging
      // keeps the current model (and its name) and just folds the template in.
      if (mode === "replace") setModelName(templateModelName(pendingTemplate.name));
      applyTemplate(pendingTemplate.graph, mode);
    }
    setPendingTemplate(null);
    setShowLibrary(false);
  }, [pendingTemplate, applyTemplate]);

  // ── Canvas class based on tool ─────────────────────────────────────────────
  const canvasClass = [
    tool === "add" ? "canvas-add" : tool === "connect" ? "canvas-connect" : "",
    layoutAnimating ? "canvas-animating" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className="flex flex-col h-screen overflow-hidden bg-[#f7f8fa]"
      style={{ fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif" }}
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      <TopBar
        onImport={() => setShowImport(true)}
        onExport={handleExport}
        onExportSvg={handleExportSvg}
        exportDisabled={graph.nodes.length === 0}
        onShare={handleShare}
        shareDisabled={graph.nodes.length === 0}
        onLibrary={() => setShowLibrary(true)}
        onOpenGoal={() => setShowGoal(true)}
        goalSet={!!goal}
      />
      {shareToast && <ShareToast message={shareToast} onClose={() => setShareToast(null)} />}
      {showImport && (
        <ImportDialog
          onConfirm={handleImportConfirm}
          onClose={() => setShowImport(false)}
        />
      )}
      {showClear && (
        <ClearCanvasDialog
          counts={{ marts: graph.nodes.length, relationships: graph.edges.length }}
          onDelete={clearCanvas}
          onExportAndDelete={handleExportAndClear}
          onClose={() => setShowClear(false)}
        />
      )}
      {showWelcome && (
        <WelcomeDialog
          onUseTemplate={(g, name) => { handleUseTemplate(g, name); setShowWelcome(false); }}
          onStartBlank={() => setShowWelcome(false)}
          onImport={() => { setShowWelcome(false); setShowImport(true); }}
        />
      )}
      {showLibrary && (
        <LibraryDialog
          onUse={handleUseTemplate}
          onClose={() => setShowLibrary(false)}
        />
      )}
      {pendingTemplate && (
        <TemplateApplyDialog
          graph={pendingTemplate.graph}
          name={pendingTemplate.name}
          onConfirm={handleTemplateApplyConfirm}
          onClose={() => setPendingTemplate(null)}
        />
      )}
      {showGoal && (
        <GoalDialog
          current={goal}
          onConfirm={g => { setGoalState(g); persistGoal(g); }}
          onClear={() => { setGoalState(null); persistGoal(null); setShowGoal(false); }}
          onClose={() => setShowGoal(false)}
        />
      )}
      <div className="flex flex-1 min-h-0 relative">
        {/* React Flow canvas */}
        <div
          className={`flex-1 relative ${canvasClass}`}
          onDoubleClick={handleWrapperDoubleClick}
        >
          {/* Tool dock — anchored to the canvas (not the outer row) so it sits
              just inside the canvas edge and slides over as the rail opens. */}
          <Dock activeTool={tool} onToolChange={handleToolChange} viewMode={viewMode} onToggleView={handleToggleView} onClear={() => setShowClear(true)} clearDisabled={graph.nodes.length === 0} relLabelMode={relLabelMode} onRelLabelModeChange={handleRelLabelModeChange} />
          <ReactFlow
            nodes={rfNodes}
            edges={rfEdges}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onRfEdgesChange}
            onConnect={onConnect}
            onReconnect={onReconnect}
            edgesReconnectable={false}
            onPaneClick={onPaneClick}
            onNodeClick={onNodeClick}
            onEdgeClick={onEdgeClick}
            connectionMode={ConnectionMode.Loose}
            fitView={false}
            minZoom={0.4}
            maxZoom={1.6}
            nodesDraggable={tool === "select"}
            nodesConnectable={true}
            selectNodesOnDrag={false}
            panOnDrag={tool === "select"}
            zoomOnScroll={true}
            zoomOnDoubleClick={false}
            deleteKeyCode={null}
          >
            <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} color="#e2e6ec" />
            {/* Nudged up to leave room for the feedback link directly below. */}
            <Controls position="bottom-left" style={{ bottom: 60, left: 15, margin: 0 }} />
          </ReactFlow>

          {/* Feedback link — bottom-left, directly under the zoom controls.
              Opens the Google Form in a new tab. */}
          <a
            href="https://forms.gle/CRLzZzdvHRqErkfG7"
            target="_blank"
            rel="noreferrer"
            title="Share your feedback on Model Canvas"
            className="absolute bottom-[16px] left-[15px] z-[5] flex items-center gap-[6px] rounded-lg bg-white/90 px-[10px] py-[6px] text-[12px] font-[550] text-slate-500 shadow-[0_1px_3px_rgba(15,23,42,0.1)] backdrop-blur-sm transition-colors hover:text-slate-900"
          >
            <MessageSquare size={14} /> Feedback
          </a>

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

        {/* Right region: a unified Sheet hosting the active panel + the always-on icon rail */}
        <ModelSheet
          active={panel.active}
          modal={panel.active !== "inspect"}
          title={SHEET_TITLES[panel.active ?? "inspect"]}
          onClose={() => { const wasInspect = panel.active === "inspect"; panel.close(); if (wasInspect) setSelection(null); }}
        >
          {panel.active === "inspect" && (
            <Inspector
              selection={selection}
              nodes={graph.nodes}
              edges={graph.edges}
              onUpdateNode={store.updateNode}
              onUpdateEdge={store.updateEdge}
              onClose={() => { setSelection(null); panel.close(); }}
              profileName="uml-domain"
              embedded
            />
          )}
          {panel.active === "share" && (
            <SharePanel
              shareUrl={buildShareUrl(store.get(), modelName)}
              onCopy={() => void handleShare()}
              onExportImage={handleExportSvg}
            />
          )}
        </ModelSheet>
        <RightRail active={panel.active} onOpen={panel.open} />
      </div>
    </div>
  );
}

// ── Share confirmation toast (auto-dismisses) ─────────────────────────────────
function ShareToast({ message, onClose }: { message: string; onClose: () => void }) {
  useEffect(() => {
    const t = setTimeout(onClose, 3500);
    return () => clearTimeout(t);
  }, [onClose]);
  return (
    <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-xl border border-emerald-300 bg-white px-4 py-3 text-[13px] shadow-2xl">
      <span className="h-2 w-2 rounded-full bg-emerald-500 flex-shrink-0" />
      <span className="text-slate-800">{message}</span>
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
