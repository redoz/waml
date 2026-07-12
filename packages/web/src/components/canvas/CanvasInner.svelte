<script lang="ts">
  // Mirrors packages/web/src/components/canvas/Canvas.tsx's `CanvasInner`
  // (L141-612) — the canvas orchestrator: all state, derived values, effects,
  // handlers and the SvelteFlow element + chrome.
  import {
    SvelteFlow,
    Background,
    BackgroundVariant,
    Controls,
    ConnectionMode,
    useSvelteFlow,
    type Node,
    type Edge,
    type Connection,
    type Viewport,
  } from "@xyflow/svelte";
  import { MessageSquare } from "lucide-svelte";

  import { model, store } from "../../state/model.svelte";
  import { sharedModelName, isFirstVisit, onStoreError } from "../../state/bootstrap";
  import { runDagreLayout, NODE_W, NODE_H } from "../../canvas/layout";
  import { toRFNode } from "./toRFNode";
  import { nodeTypes, edgeTypes } from "./flowTypes";
  import { buildRfEdges, buildAnchorEdges } from "./edges";
  import {
    type SelectionSet,
    EMPTY_SELECTION,
    isSelectionEmpty,
    focusedSelection,
    selectionFromFlow,
    anchorNodeIds,
    deleteSelection,
  } from "./selection";
  import Dock, { type Tool } from "./Dock.svelte";
  import SelectionToolbar from "./SelectionToolbar.svelte";
  import { matchesShortcut } from "../../lib/shortcuts";

  import TopBar from "../TopBar.svelte";
  import ImportDialog from "../ImportDialog.svelte";
  import ClearCanvasDialog from "../ClearCanvasDialog.svelte";
  import WelcomeDialog from "../WelcomeDialog.svelte";
  import LibraryDialog from "../LibraryDialog.svelte";
  import TemplateApplyDialog from "../TemplateApplyDialog.svelte";
import ShareToast from "../ShareToast.svelte";
  import ShareDialog from "../share/ShareDialog.svelte";
  import Inspector from "../inspector/Inspector.svelte";
  import ExternalRefs from "../inspector/ExternalRefs.svelte";
  import InspectorPanel from "../inspector/InspectorPanel.svelte";
  import EdgeFlag from "../chrome/EdgeFlag.svelte";

  import {
    effectiveDiagrams,
    ALL_DIAGRAM_KEY,
    loadActiveDiagramKey,
    persistActiveDiagramKey,
  } from "@uaml/core/state/diagrams";
  import { resolveDisplay, type DiagramDisplay } from "@uaml/okf";
  import { loadModelName, persistModelName, DEFAULT_MODEL_NAME, templateModelName } from "@uaml/core/state/modelName";
  import { persistBundle } from "@uaml/core/state/persist";
  import { bundleToDownloadFiles, downloadBundle } from "@uaml/core/okf/io";
  import { buildShareUrl } from "@uaml/core/share/url";
  import { exportCanvasSvg, buildCanvasSvg } from "@uaml/core/share/exportImage";
  import { svgToPngBlob } from "../../share/rasterize";
  import { mergeBundles } from "@uaml/core/sync/merge";
  import type { Bundle } from "@uaml/core/state/model";

  // ── State (one $state per React useState) ───────────────────────────────────
  // Full multi-selection (node keys + model edge ids). SvelteFlow owns the live
  // click/shift-click/marquee selection and reports it via onselectionchange; we
  // mirror it here so it drives the toolbar, delete, and — via `focused` — the
  // single-element Inspector.
  let selectionSet = $state<SelectionSet>(EMPTY_SELECTION);
  // Bound to SvelteFlow's viewport so the toolbar re-anchors on pan/zoom.
  let viewport = $state<Viewport>();
  // Screen anchor for the floating SelectionToolbar (null ⇒ hidden).
  let toolbarPos = $state<{ x: number; y: number } | null>(null);
  let tool = $state<Tool>("select");
  // True briefly during auto-layout so nodes glide (CSS transition) to their new
  // positions instead of snapping.
  let layoutAnimating = $state(false);
  // Computed once at mount (mirrors React's useState initializer, evaluated only
  // on first render): effectiveDiagrams($model) synthesizes the implicit "All"
  // diagram when the model has none yet.
  let activeDiagramKey = $state<string>(loadActiveDiagramKey() ?? effectiveDiagrams($model)[0].key);
  // A shared link's name wins on first load (opening someone's named model);
  // otherwise restore the locally-persisted name.
  let modelName = $state(sharedModelName ?? loadModelName());
  let showImport = $state(false);
  let showLibrary = $state(false);
  // A template chosen from the library while the canvas already had content —
  // held until the user confirms Replace vs Merge in the TemplateApplyDialog.
  let pendingTemplate = $state<{ bundle: Bundle; name: string } | null>(null);
  // First-screen chooser — shown once to brand-new visitors (no persisted model).
  let showWelcome = $state(isFirstVisit);
  let shareToast = $state<string | null>(null);
  let showClear = $state(false);
  // Modal Share dialog (link + share-as-image). Replaces the old rail Share panel.
  let showShare = $state(false);

  // Inspector pin state. The panel is always mounted (never closes); an empty
  // selection rests as a compact bar + hint. When pinned it dims (translucent)
  // while idle, fading back opaque on hover/focus.
  let inspectorPinned = $state(false);
  // Bound to the InspectorPanel's resizable width so the edge-flags can slide
  // left, clear of the open panel, instead of sitting on top of it.
  let inspectorWidth = $state(380);

  // SvelteFlow owns the live node/edge arrays so dragging follows the cursor
  // smoothly. The model store stays the source of truth: we sync store → RF on
  // structural/data changes, and write positions back to the store only at drag
  // end (onnodedragstop below).
  let rfNodes = $state<Node[]>([]);
  let rfEdges = $state<Edge[]>([]);

  // useSvelteFlow() (confirmed via hooks/useSvelteFlow.svelte.d.ts) requires flow
  // context — available because Canvas.svelte wraps this component in
  // <SvelteFlowProvider>.
  const { screenToFlowPosition, fitView, flowToScreenPosition, getNodesBounds } = useSvelteFlow();

  // ── Derived ──────────────────────────────────────────────────────────────────
  // Single "focused" element (the sole selected node/edge) for the Inspector; a
  // multi-selection focuses nothing.
  const focused = $derived(focusedSelection(selectionSet));
  // Element picker entries: the active diagram's member nodes (objects + notes).
  const inspectorOptions = $derived(
    $model.nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => ({ key: n.key, label: n.title.trim() || "Untitled" })),
  );
  const inspectorSelectedKey = $derived(focused?.type === "node" ? focused.id : null);
  const inspectorFocusedKind = $derived(focused?.type);
  const diagrams = $derived(effectiveDiagrams($model));
  const activeDiagram = $derived(diagrams.find((d) => d.key === activeDiagramKey) ?? diagrams[0]);
  // The active diagram's resolved per-diagram render settings (absent ⇒ defaults).
  // Replaces the old global viewMode/relLabelMode browser preferences.
  const activeDisplay = $derived(resolveDisplay(activeDiagram.display));
  const memberSet = $derived(new Set(activeDiagram.members));
  const imageName = $derived(modelName.trim() || "model");
  const canvasClass = $derived(
    [tool === "add" ? "canvas-add" : tool === "connect" ? "canvas-connect" : "", layoutAnimating ? "canvas-animating" : ""]
      .filter(Boolean)
      .join(" "),
  );

  // ── Effects (mirror React useEffects) ───────────────────────────────────────
  // 1) Rebuild rfNodes from the model, filtered to the active diagram's members.
  $effect(() => {
    const nodes = $model.nodes;
    const disp = activeDisplay;
    const diag = activeDiagram;
    const selNodes = selectionSet.nodes;
    rfNodes = nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => ({
        ...toRFNode(n, disp, diag.profile, diag.hints?.collapse?.includes(n.key) ?? false),
        selected: selNodes.includes(n.key),
      }));
  });

  // 2) Rebuild rfEdges from the model's visible edges + anchor edges, folding in
  // the selection-driven zIndex elevation (Canvas.tsx L199-223 combined into one
  // assignment, per the brief). NB: @xyflow/svelte's EdgeBase (confirmed via
  // node_modules/.../@xyflow+system@.../dist/esm/types/edges.d.ts) has no
  // per-edge `reconnectable` field and SvelteFlowProps has no `edgesReconnectable`
  // flow prop (both React-Flow-only) — reconnect scoping to the selected edge is
  // instead handled by RelEdge's own `selected` prop gating <EdgeReconnectAnchor>.
  $effect(() => {
    const nodes = $model.nodes;
    const edges = $model.edges;
    const disp = activeDisplay;
    const visibleNodes = nodes.filter((n) => memberSet.has(n.key));
    const visibleEdges = edges.filter((e) => memberSet.has(e.from) && memberSet.has(e.to));
    const selEdges = selectionSet.edges;
    rfEdges = [...buildRfEdges(visibleEdges, nodes, disp), ...buildAnchorEdges(visibleNodes, visibleEdges)].map(
      (e) => {
        const modelEdgeId = (e.data as { modelEdgeId?: string } | undefined)?.modelEdgeId;
        const isSelected = modelEdgeId != null && selEdges.includes(modelEdgeId);
        return { ...e, zIndex: isSelected ? 1000 : 0, selected: isSelected };
      },
    );
  });

  // 3) Persist the active diagram key on change.
  $effect(() => {
    persistActiveDiagramKey(activeDiagram.key);
  });

  // 4) Toolbar anchor: screen position of the top-center of the selection's
  // bounding box. Depends on the selection, the live node positions (rfNodes) and
  // the viewport, so it re-anchors on drag/pan/zoom. Client coords → the toolbar
  // is position:fixed. Falls back to a top-center default if bounds can't be
  // measured yet (e.g. brand-new node before layout).
  $effect(() => {
    const set = selectionSet;
    void viewport;
    void rfNodes;
    if (isSelectionEmpty(set)) {
      toolbarPos = null;
      return;
    }
    const wanted = anchorNodeIds(set, $model.edges);
    const present = wanted.filter((id) => rfNodes.some((n) => n.id === id));
    try {
      const bounds = getNodesBounds(present.length > 0 ? present : wanted);
      if (bounds && Number.isFinite(bounds.x) && Number.isFinite(bounds.width) && bounds.width >= 0) {
        const p = flowToScreenPosition({ x: bounds.x + bounds.width / 2, y: bounds.y });
        if (Number.isFinite(p.x) && Number.isFinite(p.y)) {
          toolbarPos = { x: p.x, y: p.y };
          return;
        }
      }
    } catch {
      // fall through to the default below
    }
    toolbarPos = { x: window.innerWidth / 2, y: 120 };
  });

  // 5) Persist the model name on change.
  $effect(() => {
    persistModelName(modelName);
  });

  // 6) Mirror the bundle to localStorage on every change so a refresh/crash
  // doesn't lose work. `$model` is the reactive trigger; the bundle is the truth.
  $effect(() => {
    void $model;
    persistBundle(store.getBundle());
  });

  // Surface a rejected `apply_ops` edit (e.g. a name collision) as a toast rather
  // than letting it throw out of a handler.
  $effect(() => onStoreError((e) => { shareToast = e; }));

  // Share-confirmation toast auto-dismiss now lives in the <ShareToast> component
  // (Task 3), which owns its own setTimeout(onClose, 3500) effect — mirrors
  // React's <ShareToast>. No inline effect needed here.

  // ── Drag write-back ──────────────────────────────────────────────────────────
  // Confirmed via node_modules/.../@xyflow/svelte/dist/lib/types/events.d.ts:
  //   onnodedragstop?: NodeTargetEventWithPointer = ({ targetNode, nodes, event }) => void
  // This is the idiomatic drag-end write-back (SvelteFlow mutates `rfNodes` in
  // place during drag via bind:nodes; the store is the source of truth, so we
  // persist the final position(s) here instead of on every drag tick).
  function onNodeDragStop({ targetNode, nodes }: { targetNode: Node | null; nodes: Node[] }) {
    const moved = nodes ?? (targetNode ? [targetNode] : []);
    for (const n of moved) store.updateNode(n.id, { position: { x: n.position.x, y: n.position.y } });
  }

  // ── Reconnect / connect ──────────────────────────────────────────────────────
  // onreconnect: OnReconnect<Edge> = (oldEdge, newConnection) => void (confirmed
  // via @xyflow/system dist/esm/types/general.d.ts).
  function onReconnect(oldEdge: Edge, conn: Connection) {
    if (!conn.source || !conn.target || conn.source === conn.target) return;
    store.updateEdge(oldEdge.id, { from: conn.source, to: conn.target, sourceHandle: conn.sourceHandle, targetHandle: conn.targetHandle });
  }

  // onconnect: OnConnect = (connection: Connection) => void.
  function onConnect(connection: Connection) {
    if (!connection.source || !connection.target) return;
    // Select the freshly-drawn edge (shows the toolbar). The Inspector no longer
    // auto-opens on selection — open it via the Inspect flag to set join keys.
    const e = store.addEdge(connection.source, connection.target, connection.sourceHandle, connection.targetHandle);
    if (e) selectionSet = { nodes: [], edges: [e.id] };
  }

  // ── Selection change ───────────────────────────────────────────────────────
  // onselectionchange: ({ nodes, edges }) => void. SvelteFlow owns the live
  // selection (plain click, Shift/Ctrl-click accumulation via multiSelectionKey,
  // and the drag marquee via selectionOnDrag); we mirror the result into our
  // model-keyed set (collapsing ERD's per-model-edge RF edges).
  function onSelectionChange({ nodes, edges }: { nodes: Node[]; edges: Edge[] }) {
    selectionSet = selectionFromFlow(nodes, edges);
  }

  // ── Pane click → add (in Add tool) ─────────────────────────────────────────
  // onpaneclick: ({ event }: { event: MouseEvent }) => void. Deselection on a
  // plain pane click is handled by SvelteFlow itself (→ onselectionchange).
  function onPaneClick({ event }: { event: MouseEvent }) {
    if (tool === "add") {
      const pos = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      const n = store.addNode(
        { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
        activeDiagram.key === ALL_DIAGRAM_KEY ? undefined : activeDiagram.key,
      );
      selectionSet = { nodes: [n.key], edges: [] };
      tool = "select";
    }
  }

  // ── Auto-layout + tool handler ─────────────────────────────────────────────
  function handleToolChange(t: Tool) {
    if (t === "layout") {
      const { nodes, edges } = store.get();
      const positions = runDagreLayout(nodes, edges, activeDisplay);
      // Turn on node transitions, move everything, then frame the result — so
      // the model visibly "organizes itself" instead of snapping. Cleared after
      // the glide so dragging stays instant.
      layoutAnimating = true;
      positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
      setTimeout(() => fitView({ duration: 500, padding: 0.18 }), 30);
      setTimeout(() => {
        layoutAnimating = false;
      }, 560);
      return;
    }
    tool = t;
  }

  // Merge a single-field edit into the active diagram's display and persist it on
  // the diagram (per-diagram, not per-browser). For the implicit "All" diagram
  // (no real diagram in the model yet) updateDiagram is a no-op, mirroring rename.
  function handleDisplayChange(p: Partial<DiagramDisplay>) {
    store.updateDiagram(activeDiagram.key, { display: { ...activeDisplay, ...p } });
  }

  // ── Keyboard delete ────────────────────────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    if (matchesShortcut("selection.delete", e) && !isSelectionEmpty(selectionSet)) {
      const tag = (e.target as HTMLElement).tagName;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
      handleDeleteSelection();
    }
  }

  // ── Selection-toolbar actions ──────────────────────────────────────────────
  // Remove every selected node + edge (shared by the Delete key and the toolbar's
  // "Delete selection").
  function handleDeleteSelection() {
    deleteSelection(store, selectionSet);
    selectionSet = EMPTY_SELECTION;
  }

  // "New diagram from selection": seed a diagram with EXACTLY the selected node
  // ids (edges follow implicitly via membership) and activate it. Disabled by the
  // toolbar when no nodes are selected, so `nodes` is non-empty here.
  function handleNewDiagramFromSelection(name: string) {
    const d = store.addDiagramFromMembers(name, selectionSet.nodes);
    activeDiagramKey = d.key;
    selectionSet = EMPTY_SELECTION;
  }

  // ── Double-click on empty pane → add node (works in any tool) ──────────────
  // Hit-test uses .svelte-flow__* (SvelteFlow's DOM class prefix, confirmed via
  // canvas.css which was already renamed from .react-flow__* for this port).
  function handleWrapperDoubleClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest(".svelte-flow__node") || target.closest(".svelte-flow__edge")) return;
    if (target.closest("[data-dock]")) return; // double-clicking the toolbar shouldn't drop a node
    const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const n = store.addNode(
      { x: position.x - NODE_W / 2, y: position.y - NODE_H / 2 },
      activeDiagram.key === ALL_DIAGRAM_KEY ? undefined : activeDiagram.key,
    );
    selectionSet = { nodes: [n.key], edges: [] };
    tool = "select";
  }

  // ── Import / Export / Share handlers ───────────────────────────────────────
  function handleExport() {
    const title = modelName.trim() || "model-okf";
    const files = bundleToDownloadFiles(store.getBundle(), title);
    downloadBundle(files, title);
  }

  // Clear the canvas: permanently wipe every node + edge. No undo — the dialog
  // warns and offers an OKF export first.
  function clearCanvas() {
    store.load([]);
    selectionSet = EMPTY_SELECTION;
    showClear = false;
    modelName = DEFAULT_MODEL_NAME;
  }

  function handleExportAndClear() {
    handleExport();
    clearCanvas();
  }

  // Export the canvas as an SVG (whole model, UAML watermark). Uses the live RF
  // node list (measured sizes) to frame the export. exportCanvasSvg's 3rd arg
  // (viewportSelector) was made required by Plan 1; Svelte passes the
  // `.svelte-flow__` viewport class.
  function handleExportSvg() {
    exportCanvasSvg(rfNodes, imageName, ".svelte-flow__viewport").catch(() => {
      shareToast = "Couldn't export the image — please try again.";
    });
  }

  // Render the current diagram to a PNG for the Share dialog's "Share as image"
  // flow: reuse the SVG export path (buildCanvasSvg → styles inlined), then
  // rasterize that SVG onto a canvas. Returns null when there's nothing to draw.
  async function generateSharePng(): Promise<Blob | null> {
    const built = await buildCanvasSvg(rfNodes, ".svelte-flow__viewport");
    if (!built) return null;
    return svgToPngBlob(built.svg, { width: built.width, height: built.height });
  }

  // Dagre-lay out the current (derived) model and feed positions into the store's
  // overlay. The OKF bundle carries no positions, so without this every freshly
  // loaded node piles up at the origin.
  function layoutAll() {
    const g = store.get();
    const positions = runDagreLayout(g.nodes, g.edges, activeDisplay);
    positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
  }

  // Replace the whole model with a bundle, then auto-layout it.
  function loadBundleWithLayout(bundle: Bundle) {
    store.load(bundle);
    layoutAll();
  }

  // Merge an incoming bundle into the canvas (bundle-native slug-keyed concat),
  // then re-layout. Stage 1b re-lays-out the whole model on merge (positions live
  // in the overlay, not the bundle).
  function applyMergeWithLayout(bundle: Bundle) {
    store.load(mergeBundles(store.getBundle(), bundle));
    layoutAll();
  }

  function handleImportConfirm(bundle: Bundle, mode: "replace" | "merge") {
    if (mode === "merge") applyMergeWithLayout(bundle);
    else loadBundleWithLayout(bundle);
    showImport = false;
  }

  function applyTemplate(bundle: Bundle, mode: "replace" | "merge") {
    if (mode === "merge") applyMergeWithLayout(bundle);
    else loadBundleWithLayout(bundle);
  }

  function handleUseTemplate(bundle: Bundle, name: string) {
    // Empty canvas → drop the template straight in. Non-empty → ask Replace vs
    // Merge first so existing work isn't silently wiped.
    if (store.get().nodes.length === 0) {
      modelName = templateModelName(name); // "My {template} model"
      applyTemplate(bundle, "replace");
      showLibrary = false;
    } else {
      pendingTemplate = { bundle, name };
    }
  }

  function handleTemplateApplyConfirm(mode: "replace" | "merge") {
    if (pendingTemplate) {
      // Replacing = a fresh model from this template, so re-seed the name;
      // merging keeps the current model (and its name) and just folds the
      // template in.
      if (mode === "replace") modelName = templateModelName(pendingTemplate.name);
      applyTemplate(pendingTemplate.bundle, mode);
    }
    pendingTemplate = null;
    showLibrary = false;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="flex flex-col h-screen overflow-hidden bg-[#f7f8fa]"
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
  onkeydown={handleKeyDown}
  tabindex="0"
>
  <TopBar
    onImport={() => (showImport = true)}
    onExport={handleExport}
    onExportSvg={handleExportSvg}
    exportDisabled={$model.nodes.length === 0}
    onShare={() => (showShare = true)}
    onLibrary={() => (showLibrary = true)}
    diagrams={diagrams}
    activeDiagramKey={activeDiagram.key}
    onSelectDiagram={(key) => (activeDiagramKey = key)}
    onRenameDiagram={(title) => store.updateDiagram(activeDiagram.key, { title })}
    onCreateDiagram={(name) => {
      const d = store.addDiagram(name);
      activeDiagramKey = d.key;
    }}
  />

  {#if shareToast}
    <ShareToast message={shareToast} onClose={() => (shareToast = null)} />
  {/if}

  {#if showImport}
    <ImportDialog onConfirm={handleImportConfirm} onClose={() => (showImport = false)} />
  {/if}
  {#if showClear}
    <ClearCanvasDialog
      counts={{ marts: $model.nodes.length, relationships: $model.edges.length }}
      onDelete={clearCanvas}
      onExportAndDelete={handleExportAndClear}
      onClose={() => (showClear = false)}
    />
  {/if}
  {#if showWelcome}
    <WelcomeDialog
      onUseTemplate={(g, name) => {
        handleUseTemplate(g, name);
        showWelcome = false;
      }}
      onStartBlank={() => (showWelcome = false)}
      onImport={() => {
        showWelcome = false;
        showImport = true;
      }}
    />
  {/if}
  {#if showLibrary}
    <LibraryDialog onUse={handleUseTemplate} onClose={() => (showLibrary = false)} />
  {/if}
  {#if pendingTemplate}
    <TemplateApplyDialog
      bundle={pendingTemplate.bundle}
      name={pendingTemplate.name}
      onConfirm={handleTemplateApplyConfirm}
      onClose={() => (pendingTemplate = null)}
    />
  {/if}
  {#if showShare}
    <ShareDialog
      shareUrl={buildShareUrl(store.getBundle(), modelName)}
      imageName={imageName}
      canShareImage={$model.nodes.length > 0}
      generatePng={generateSharePng}
      onClose={() => (showShare = false)}
    />
  {/if}

  <div class="flex flex-1 min-h-0 relative">
    <!-- SvelteFlow canvas -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="flex-1 relative {canvasClass}" data-canvas-wrapper ondblclick={handleWrapperDoubleClick}>
      <!-- Tool dock — anchored to the canvas (not the outer row) so it sits just
           inside the canvas edge and slides over as the rail opens. The diagram
           switcher now lives in the TopBar title control. -->
      <Dock
        activeTool={tool}
        onToolChange={handleToolChange}
        onClear={() => (showClear = true)}
        clearDisabled={$model.nodes.length === 0}
        display={activeDisplay}
        onDisplayChange={handleDisplayChange}
      />
      <SvelteFlow
        bind:nodes={rfNodes}
        bind:edges={rfEdges}
        {nodeTypes}
        {edgeTypes}
        bind:viewport={viewport}
        onnodedragstop={onNodeDragStop}
        onconnect={onConnect}
        onreconnect={onReconnect}
        onpaneclick={onPaneClick}
        onselectionchange={onSelectionChange}
        connectionMode={ConnectionMode.Loose}
        fitView={false}
        minZoom={0.4}
        maxZoom={1.6}
        nodesDraggable={tool === "select"}
        nodesConnectable={true}
        selectNodesOnDrag={false}
        selectionOnDrag={tool === "select"}
        selectionKey="Shift"
        multiSelectionKey={["Meta", "Control", "Shift"]}
        panActivationKey="Space"
        panOnDrag={tool === "select" ? [1, 2] : false}
        zoomOnScroll={true}
        zoomOnDoubleClick={false}
        deleteKey={null}
      >
        <!-- Background color prop is `patternColor` (confirmed via
             plugins/Background/types.d.ts — BackgroundProps has bgColor +
             patternColor, no `color`). -->
        <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} patternColor="#e2e6ec" />
        <!-- Controls `position` accepts PanelPosition ("bottom-left" etc.),
             confirmed via @xyflow/system dist/esm/types/general.d.ts. The
             feedback link moved to a right-edge flag, so the zoom controls
             return to their normal bottom-left resting position. -->
        <Controls position="bottom-left" style="bottom:15px;left:15px;margin:0;" />
      </SvelteFlow>

      <!-- Empty canvas CTA -->
      {#if $model.nodes.length === 0}
        <div
          class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-center text-slate-500 pointer-events-none z-[1]"
          style="font-size:15px;"
        >
          <div><strong class="text-slate-900">Empty canvas</strong></div>
          <div class="mt-[6px] text-[13px] leading-[1.6]">
            Double-click anywhere to add an object.<br />
            Drag from a node's port to create a relationship.
          </div>
        </div>
      {/if}

      <!-- Floating toolbar anchored above the selection's bounding box. Shown
           whenever ≥1 element is selected. -->
      {#if !isSelectionEmpty(selectionSet) && toolbarPos}
        <SelectionToolbar
          x={toolbarPos.x}
          y={toolbarPos.y}
          nodeCount={selectionSet.nodes.length}
          edgeCount={selectionSet.edges.length}
          onNewDiagram={handleNewDiagramFromSelection}
          onDelete={handleDeleteSelection}
        />
      {/if}
    </div>

    <!-- Right-edge Feedback flag; slides left by the panel width to stay clear
         of the always-present Inspector. -->
    <EdgeFlag
      label="Feedback"
      offset={62}
      rightOffset={inspectorWidth}
      href="https://github.com/redoz/uaml/issues/new"
    >
      {#snippet icon()}<MessageSquare size={16} />{/snippet}
    </EdgeFlag>

    <!-- Always-present floating Inspector (translucent when pinned + idle). -->
    <InspectorPanel
      options={inspectorOptions}
      selectedKey={inspectorSelectedKey}
      focusedKind={inspectorFocusedKind}
      onSelect={(key) => (selectionSet = key ? { nodes: [key], edges: [] } : EMPTY_SELECTION)}
      pinned={inspectorPinned}
      bind:width={inspectorWidth}
      onTogglePin={() => (inspectorPinned = !inspectorPinned)}
    >
      <Inspector
        selection={focused}
        nodes={$model.nodes}
        edges={$model.edges}
        onUpdateNode={store.updateNode}
        onUpdateEdge={store.updateEdge}
        onClose={() => {
          selectionSet = EMPTY_SELECTION;
        }}
        profileName={activeDiagram.profile}
        embedded
      >
        {#snippet externalRefs()}
          {#if focused?.type === "node"}
            <ExternalRefs
              nodeKey={focused.id}
              nodes={$model.nodes}
              edges={$model.edges}
              members={activeDiagram.members}
              diagrams={diagrams}
              onNavigate={(diagramKey, nodeKey) => {
                activeDiagramKey = diagramKey;
                selectionSet = { nodes: [nodeKey], edges: [] };
              }}
            />
          {/if}
        {/snippet}
      </Inspector>
    </InspectorPanel>
  </div>
</div>
