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
  } from "@xyflow/svelte";
  import { MessageSquare } from "lucide-svelte";

  import { model, store } from "../../state/model.svelte";
  import { sharedModelName, isFirstVisit } from "../../state/bootstrap";
  import { runDagreLayout, NODE_W, NODE_H } from "../../canvas/layout";
  import { toRFNode } from "./toRFNode";
  import { nodeTypes, edgeTypes } from "./flowTypes";
  import { buildRfEdges, buildAnchorEdges } from "./edges";
  import type { Selection } from "./selection";
  import Dock, { type Tool } from "./Dock.svelte";
  import DiagramTabs from "./DiagramTabs.svelte";

  import TopBar from "../TopBar.svelte";
  import ImportDialog from "../ImportDialog.svelte";
  import ClearCanvasDialog from "../ClearCanvasDialog.svelte";
  import WelcomeDialog from "../WelcomeDialog.svelte";
  import LibraryDialog from "../LibraryDialog.svelte";
  import TemplateApplyDialog from "../TemplateApplyDialog.svelte";
  import GoalDialog from "../GoalDialog.svelte";
  import Inspector from "../inspector/Inspector.svelte";
  import ExternalRefs from "../inspector/ExternalRefs.svelte";
  import ModelSheet from "../rail/ModelSheet.svelte";
  import RightRail from "../rail/RightRail.svelte";
  import SharePanel from "../rail/SharePanel.svelte";
  import { createRightPanel, type RightPanelId } from "../rail/rightPanel.svelte";

  import {
    effectiveDiagrams,
    ALL_DIAGRAM_KEY,
    loadActiveDiagramKey,
    persistActiveDiagramKey,
  } from "@mc/core/state/diagrams";
  import { getProfile } from "@mc/core/profiles";
  import { loadViewMode, persistViewMode, type ViewMode } from "@mc/core/state/viewMode";
  import { loadRelLabelMode, persistRelLabelMode, type RelLabelMode } from "@mc/core/state/relLabels";
  import { loadModelName, persistModelName, DEFAULT_MODEL_NAME, templateModelName } from "@mc/core/state/modelName";
  import { loadGoal, persistGoal, type BusinessGoal } from "@mc/core/state/goal";
  import { persistGraph } from "@mc/core/state/persist";
  import { graphToBundleFiles, downloadBundle } from "@mc/core/okf/io";
  import { buildShareUrl } from "@mc/core/share/url";
  import { exportCanvasSvg } from "@mc/core/share/exportImage";
  import { mergeGraphs } from "@mc/core/sync/merge";
  import type { ModelGraph } from "@mc/okf";

  // Titles shown in the right Sheet header per active panel.
  const SHEET_TITLES: Record<RightPanelId, string> = { inspect: "Inspect", share: "Share model" };

  // ── State (one $state per React useState) ───────────────────────────────────
  let selection = $state<Selection>(null);
  let tool = $state<Tool>("select");
  let viewMode = $state<ViewMode>(loadViewMode());
  let relLabelMode = $state<RelLabelMode>(loadRelLabelMode());
  // True briefly during auto-layout so nodes glide (CSS transition) to their new
  // positions instead of snapping.
  let layoutAnimating = $state(false);
  // Business goal — a stored objective ({niche, goal}) persisted in localStorage.
  let goal = $state<BusinessGoal | null>(loadGoal());
  // Computed once at mount (mirrors React's useState initializer, evaluated only
  // on first render): effectiveDiagrams($model) synthesizes the implicit "All"
  // diagram when the model has none yet.
  let activeDiagramKey = $state<string>(loadActiveDiagramKey() ?? effectiveDiagrams($model)[0].key);
  // A shared link's name wins on first load (opening someone's named model);
  // otherwise restore the locally-persisted name.
  let modelName = $state(sharedModelName ?? loadModelName());
  let showGoal = $state(false);
  let showImport = $state(false);
  let showLibrary = $state(false);
  // A template chosen from the library while the canvas already had content —
  // held until the user confirms Replace vs Merge in the TemplateApplyDialog.
  let pendingTemplate = $state<{ graph: ModelGraph; name: string } | null>(null);
  // First-screen chooser — shown once to brand-new visitors (no persisted model).
  let showWelcome = $state(isFirstVisit);
  let shareToast = $state<string | null>(null);
  let showClear = $state(false);

  // Single right-side panel state (which rail entry is open in the Sheet).
  const panel = createRightPanel();

  // SvelteFlow owns the live node/edge arrays so dragging follows the cursor
  // smoothly. The model store stays the source of truth: we sync store → RF on
  // structural/data changes, and write positions back to the store only at drag
  // end (onnodedragstop below).
  let rfNodes = $state<Node[]>([]);
  let rfEdges = $state<Edge[]>([]);

  // useSvelteFlow() (confirmed via hooks/useSvelteFlow.svelte.d.ts) requires flow
  // context — available because Canvas.svelte wraps this component in
  // <SvelteFlowProvider>.
  const { screenToFlowPosition, fitView } = useSvelteFlow();

  // ── Derived ──────────────────────────────────────────────────────────────────
  const diagrams = $derived(effectiveDiagrams($model));
  const activeDiagram = $derived(diagrams.find((d) => d.key === activeDiagramKey) ?? diagrams[0]);
  const profile = $derived(getProfile(activeDiagram.profile));
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
    const vm = viewMode;
    const diag = activeDiagram;
    const sel = selection;
    rfNodes = nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => ({
        ...toRFNode(n, vm, diag.profile, diag.hints?.collapse?.includes(n.key) ?? false),
        selected: sel?.type === "node" && sel.id === n.key,
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
    const vm = viewMode;
    const rlm = relLabelMode;
    const diag = activeDiagram;
    const visibleNodes = nodes.filter((n) => memberSet.has(n.key));
    const visibleEdges = edges.filter((e) => memberSet.has(e.from) && memberSet.has(e.to));
    const emphasizeMultiplicity =
      profile.emphasize.includes("multiplicity") &&
      !(diag.hints?.emphasize && !diag.hints.emphasize.includes("multiplicity"));
    const selId = selection?.type === "edge" ? selection.id : null;
    rfEdges = [...buildRfEdges(visibleEdges, nodes, vm, rlm, emphasizeMultiplicity), ...buildAnchorEdges(visibleNodes, visibleEdges)].map(
      (e) => {
        const modelEdgeId = (e.data as { modelEdgeId?: string } | undefined)?.modelEdgeId;
        const isSelected = modelEdgeId != null && modelEdgeId === selId;
        return { ...e, zIndex: isSelected ? 1000 : 0, selected: isSelected };
      },
    );
  });

  // 3) Persist the active diagram key on change.
  $effect(() => {
    persistActiveDiagramKey(activeDiagram.key);
  });

  // 4) Selecting a node/edge auto-opens the Inspect panel — preserves current UX.
  $effect(() => {
    if (selection) panel.open("inspect");
  });

  // 5) Persist the model name on change.
  $effect(() => {
    persistModelName(modelName);
  });

  // 6) Mirror the model to localStorage on every change so a refresh/crash
  // doesn't lose work.
  $effect(() => {
    persistGraph($model);
  });

  // 7) Share-confirmation toast auto-dismiss (mirrors React's <ShareToast>
  // useEffect(() => { const t = setTimeout(onClose, 3500); return () =>
  // clearTimeout(t); }, [onClose])) — implemented inline rather than as a
  // separate component since Svelte components can't be declared in-file.
  $effect(() => {
    if (!shareToast) return;
    const t = setTimeout(() => {
      shareToast = null;
    }, 3500);
    return () => clearTimeout(t);
  });

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
    // Open the new edge in the inspector right away so the user can set join
    // keys without an extra click to select the freshly-drawn line.
    const e = store.addEdge(connection.source, connection.target, connection.sourceHandle, connection.targetHandle);
    if (e) selection = { type: "edge", id: e.id };
  }

  // ── Pane click → add (in Add tool) or deselect ────────────────────────────
  // onpaneclick: ({ event }: { event: MouseEvent }) => void.
  function onPaneClick({ event }: { event: MouseEvent }) {
    if (tool === "add") {
      const pos = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      const n = store.addNode(
        { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
        activeDiagram.key === ALL_DIAGRAM_KEY ? undefined : activeDiagram.key,
      );
      selection = { type: "node", id: n.key };
      tool = "select";
      return;
    }
    selection = null;
  }

  // ── Node click → select ────────────────────────────────────────────────────
  // onnodeclick: ({ node, event }) => void.
  function onNodeClick({ node }: { node: Node }) {
    selection = { type: "node", id: node.id };
  }

  // ── Edge click → select ────────────────────────────────────────────────────
  // onedgeclick: ({ edge, event }) => void. ERD mode may render several RF edges
  // per model edge (e.g. "e1::0"); strip the suffix so the inspector still
  // selects the underlying model edge.
  function onEdgeClick({ edge }: { edge: Edge }) {
    selection = { type: "edge", id: edge.id.split("::")[0] };
  }

  // ── Auto-layout + tool handler ─────────────────────────────────────────────
  function handleToolChange(t: Tool) {
    if (t === "layout") {
      const { nodes, edges } = store.get();
      const positions = runDagreLayout(nodes, edges, viewMode);
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

  function handleToggleView() {
    const next = viewMode === "erd" ? "compact" : "erd";
    persistViewMode(next);
    viewMode = next;
  }

  // ── Keyboard delete ────────────────────────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    if ((e.key === "Delete" || e.key === "Backspace") && selection) {
      const tag = (e.target as HTMLElement).tagName;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
      if (selection.type === "node") store.removeNode(selection.id);
      else store.removeEdge(selection.id);
      selection = null;
    }
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
    selection = { type: "node", id: n.key };
    tool = "select";
  }

  // ── Import / Export / Share handlers ───────────────────────────────────────
  function handleExport() {
    const title = modelName.trim() || "model-okf";
    const files = graphToBundleFiles(store.get(), title);
    downloadBundle(files, title);
  }

  // Clear the canvas: permanently wipe every node + edge. No undo — the dialog
  // warns and offers an OKF export first.
  function clearCanvas() {
    store.set({ nodes: [], edges: [], diagrams: [] });
    selection = null;
    showClear = false;
    modelName = DEFAULT_MODEL_NAME;
  }

  function handleExportAndClear() {
    handleExport();
    clearCanvas();
  }

  // Export the canvas as an SVG (whole model, OWOX watermark). Uses the live RF
  // node list (measured sizes) to frame the export. exportCanvasSvg's 3rd arg
  // (viewportSelector) was made required by Plan 1; Svelte passes the
  // `.svelte-flow__` viewport class.
  function handleExportSvg() {
    exportCanvasSvg(rfNodes, imageName, ".svelte-flow__viewport").catch(() => {
      shareToast = "Couldn't export the image — please try again.";
    });
  }

  // Copy a shareable link that reopens this exact model. Falls back to a prompt
  // if the clipboard API is blocked (insecure context / permissions).
  async function handleShare() {
    const url = buildShareUrl(store.get(), modelName);
    const isLocal = /^(localhost|127\.|0\.0\.0\.0|\[::1\])/.test(location.hostname);
    const msg = isLocal
      ? "Link copied — note: a localhost link only opens on this machine. Deploy to share it."
      : "Link copied — anyone with it can open this model.";
    try {
      await navigator.clipboard.writeText(url);
      shareToast = msg;
    } catch {
      window.prompt("Copy this shareable link:", url);
    }
  }

  // Auto-layout a freshly loaded graph (import or template). The OKF format
  // does not persist node positions, so without this every imported node piles
  // up at the origin.
  function withLayout(g: ModelGraph): ModelGraph {
    const positions = runDagreLayout(g.nodes, g.edges, viewMode);
    return { ...g, nodes: g.nodes.map((n) => ({ ...n, position: positions.get(n.key) ?? n.position })) };
  }

  // Merge a freshly loaded graph into the canvas, laying out only the new nodes
  // so the existing layout isn't reshuffled. Shared by OKF import + templates.
  function applyMergeWithLayout(g: ModelGraph) {
    const { graph, newKeys } = mergeGraphs(store.get(), g);
    const positions = runDagreLayout(graph.nodes, graph.edges, viewMode);
    store.set({ ...graph, nodes: graph.nodes.map((n) => (newKeys.has(n.key) ? { ...n, position: positions.get(n.key) ?? n.position } : n)) });
  }

  function handleImportConfirm(g: ModelGraph, mode: "replace" | "merge") {
    if (mode === "merge") applyMergeWithLayout(g);
    else {
      const hasPositions = g.nodes.some((n) => n.position.x !== 0 || n.position.y !== 0);
      store.set(hasPositions ? g : withLayout(g));
    }
    showImport = false;
  }

  function applyTemplate(g: ModelGraph, mode: "replace" | "merge") {
    // Auto-layout the template (templates ship at 0,0).
    if (mode === "merge") applyMergeWithLayout(g);
    else store.set(withLayout(g));
  }

  function handleUseTemplate(g: ModelGraph, name: string) {
    // Empty canvas → drop the template straight in. Non-empty → ask Replace vs
    // Merge first so existing work isn't silently wiped.
    if (store.get().nodes.length === 0) {
      modelName = templateModelName(name); // "My {template} OKF with OWOX"
      applyTemplate(g, "replace");
      showLibrary = false;
    } else {
      pendingTemplate = { graph: g, name };
    }
  }

  function handleTemplateApplyConfirm(mode: "replace" | "merge") {
    if (pendingTemplate) {
      // Replacing = a fresh model from this template, so re-seed the name;
      // merging keeps the current model (and its name) and just folds the
      // template in.
      if (mode === "replace") modelName = templateModelName(pendingTemplate.name);
      applyTemplate(pendingTemplate.graph, mode);
    }
    pendingTemplate = null;
    showLibrary = false;
  }

  function handleRelLabelModeChange(mode: RelLabelMode) {
    relLabelMode = mode;
    persistRelLabelMode(mode);
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
    onShare={handleShare}
    shareDisabled={$model.nodes.length === 0}
    onLibrary={() => (showLibrary = true)}
    onOpenGoal={() => (showGoal = true)}
    goalSet={!!goal}
  />

  {#if shareToast}
    <div class="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-xl border border-emerald-300 bg-white px-4 py-3 text-[13px] shadow-2xl">
      <span class="h-2 w-2 rounded-full bg-emerald-500 flex-shrink-0"></span>
      <span class="text-slate-800">{shareToast}</span>
    </div>
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
      graph={pendingTemplate.graph}
      name={pendingTemplate.name}
      onConfirm={handleTemplateApplyConfirm}
      onClose={() => (pendingTemplate = null)}
    />
  {/if}
  {#if showGoal}
    <GoalDialog
      current={goal}
      onConfirm={(g) => {
        goal = g;
        persistGoal(g);
      }}
      onClear={() => {
        goal = null;
        persistGoal(null);
        showGoal = false;
      }}
      onClose={() => (showGoal = false)}
    />
  {/if}

  <div class="flex flex-1 min-h-0 relative">
    <!-- SvelteFlow canvas -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="flex-1 relative {canvasClass}" ondblclick={handleWrapperDoubleClick}>
      <!-- Tool dock / diagram tabs — anchored to the canvas (not the outer row)
           so they sit just inside the canvas edge and slide over as the rail opens. -->
      <DiagramTabs
        diagrams={diagrams}
        activeKey={activeDiagram.key}
        onSelect={(key) => (activeDiagramKey = key)}
        onCreate={() => {
          const name = window.prompt("Diagram name?", "New diagram");
          if (name) {
            const d = store.addDiagram(name);
            activeDiagramKey = d.key;
          }
        }}
      />
      <Dock
        activeTool={tool}
        onToolChange={handleToolChange}
        viewMode={viewMode}
        onToggleView={handleToggleView}
        onClear={() => (showClear = true)}
        clearDisabled={$model.nodes.length === 0}
        relLabelMode={relLabelMode}
        onRelLabelModeChange={handleRelLabelModeChange}
      />
      <SvelteFlow
        bind:nodes={rfNodes}
        bind:edges={rfEdges}
        {nodeTypes}
        {edgeTypes}
        onnodedragstop={onNodeDragStop}
        onconnect={onConnect}
        onreconnect={onReconnect}
        onpaneclick={onPaneClick}
        onnodeclick={onNodeClick}
        onedgeclick={onEdgeClick}
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
        deleteKey={null}
      >
        <!-- Background color prop is `patternColor` (confirmed via
             plugins/Background/types.d.ts — BackgroundProps has bgColor +
             patternColor, no `color`). -->
        <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} patternColor="#e2e6ec" />
        <!-- Controls `position` accepts PanelPosition ("bottom-left" etc.),
             confirmed via @xyflow/system dist/esm/types/general.d.ts. Nudged up
             to leave room for the feedback link directly below. -->
        <Controls position="bottom-left" style="bottom:60px;left:15px;margin:0;" />
      </SvelteFlow>

      <!-- Feedback link — bottom-left, directly under the zoom controls. Opens
           the Google Form in a new tab. -->
      <a
        href="https://forms.gle/CRLzZzdvHRqErkfG7"
        target="_blank"
        rel="noreferrer"
        title="Share your feedback on Model Canvas"
        class="absolute bottom-[16px] left-[15px] z-[5] flex items-center gap-[6px] rounded-lg bg-white/90 px-[10px] py-[6px] text-[12px] font-[550] text-slate-500 shadow-[0_1px_3px_rgba(15,23,42,0.1)] backdrop-blur-sm transition-colors hover:text-slate-900"
      >
        <MessageSquare size={14} /> Feedback
      </a>

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
    </div>

    <!-- Right region: a unified Sheet hosting the active panel + the always-on icon rail -->
    <ModelSheet
      active={panel.active}
      modal={panel.active !== "inspect"}
      title={SHEET_TITLES[panel.active ?? "inspect"]}
      onClose={() => {
        const wasInspect = panel.active === "inspect";
        panel.close();
        if (wasInspect) selection = null;
      }}
    >
      {#if panel.active === "inspect"}
        <Inspector
          selection={selection}
          nodes={$model.nodes}
          edges={$model.edges}
          onUpdateNode={store.updateNode}
          onUpdateEdge={store.updateEdge}
          onClose={() => {
            selection = null;
            panel.close();
          }}
          profileName={activeDiagram.profile}
          embedded
        >
          {#snippet externalRefs()}
            {#if selection?.type === "node"}
              <ExternalRefs
                nodeKey={selection.id}
                nodes={$model.nodes}
                edges={$model.edges}
                members={activeDiagram.members}
                diagrams={diagrams}
                onNavigate={(diagramKey, nodeKey) => {
                  activeDiagramKey = diagramKey;
                  selection = { type: "node", id: nodeKey };
                }}
              />
            {/if}
          {/snippet}
        </Inspector>
      {/if}
      {#if panel.active === "share"}
        <SharePanel shareUrl={buildShareUrl(store.get(), modelName)} onCopy={() => void handleShare()} onExportImage={handleExportSvg} />
      {/if}
    </ModelSheet>
    <RightRail active={panel.active} onOpen={panel.open} />
  </div>
</div>
