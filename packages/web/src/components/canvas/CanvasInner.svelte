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
  import { untrack } from "svelte";

  import { model, store } from "../../state/model.svelte";
  import { sharedModelName, isFirstVisit, onStoreError } from "../../state/bootstrap";
  import { runDagreLayout, NODE_W, NODE_H } from "../../canvas/layout";
  import { toRFNode } from "./toRFNode";
  import { diagramCandidateStereotypes, isDiagramEditable } from "./diagramProps";
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
  import InspectorReadonly from "../inspector/InspectorReadonly.svelte";
  import ExternalRefs from "../inspector/ExternalRefs.svelte";
  import InspectorPanel from "../inspector/InspectorPanel.svelte";
  import NavigatorPanel from "../NavigatorPanel.svelte";
  import EdgeFlag from "../chrome/EdgeFlag.svelte";
  import CentralEditPanelHost, { type CentralPanelState } from "../central/CentralEditPanelHost.svelte";
  import FlowView from "./flow/FlowView.svelte";
  import SequenceView from "./sequence/SequenceView.svelte";

  import {
    effectiveDiagrams,
    defaultDiagramKey,
    ALL_DIAGRAM_KEY,
    loadActiveDiagramKey,
    persistActiveDiagramKey,
  } from "@waml/core/state/diagrams";
  import { resolveDisplay, slugify, type DiagramDisplay, type Diagram, type ModelEdge } from "@waml/okf";
  import { getProfile } from "@waml/core/profiles";
  import { loadModelName, persistModelName, DEFAULT_MODEL_NAME, templateModelName } from "@waml/core/state/modelName";
  import { persistBundle } from "@waml/core/state/persist";
  import { bundleToDownloadFiles, downloadBundle } from "@waml/core/okf/io";
  import { buildShareUrl } from "@waml/core/share/url";
  import { exportCanvasSvg, buildCanvasSvg } from "@waml/core/share/exportImage";
  import { svgToPngBlob } from "../../share/rasterize";
  import { mergeBundles } from "@waml/core/sync/merge";
  import type { Bundle } from "@waml/core/state/model";

  // Shared with the SvelteFlow instance's own minZoom/maxZoom below, and with
  // the magnifying-glass effect's zoom clamp, so both never drift apart.
  const CANVAS_MIN_ZOOM = 0.4;
  const CANVAS_MAX_ZOOM = 1.6;

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
  // Whether the pointer is currently over the canvas wrapper — the
  // SelectionToolbar should only show while the canvas is hovered, not just
  // because a selection exists (e.g. after switching away from the canvas).
  let canvasHovered = $state(false);
  let tool = $state<Tool>("select");
  // True briefly during auto-layout so nodes glide (CSS transition) to their new
  // positions instead of snapping.
  let layoutAnimating = $state(false);
  // Computed once at mount (mirrors React's useState initializer, evaluated only
  // on first render): effectiveDiagrams($model) synthesizes the implicit "All"
  // diagram when the model has none yet.
  let activeDiagramKey = $state<string>(loadActiveDiagramKey() ?? defaultDiagramKey($model));
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
  // selection rests as a compact bar + hint. Pinned = forced solid; unpinned
  // dims (translucent) while idle, fading back opaque on hover/focus. Defaults
  // pinned so the out-of-the-box inspector stays solid.
  let inspectorPinned = $state(true);
  // Bound to the InspectorPanel's resizable width so the edge-flags can slide
  // left, clear of the open panel, instead of sitting on top of it.
  let inspectorWidth = $state(380);
  // Navigator panel — session-local, like the inspector state above. `navMode`
  // is remembered across close/reopen; unpinning returns to "centered".
  let navOpen = $state(false);
  let navMode = $state<"centered" | "docked">("centered");
  let navWidth = $state(340);
  let navCollapsed = $state(false);
  // central edit panel's current target (null = closed). Element context is
  // opened by navigator's "View / edit properties"; diagram context by
  // Dock sliders button (Task 5).
  let centralPanel = $state<CentralPanelState | null>(null);
  // The central panel's transparent preview cutout (bound down through
  // CentralEditPanelHost/CentralEditPanel) and the canvas's own wrapper —
  // both needed to compute the pan/zoom that frames the focal node/edge
  // inside the cutout's screen rect (see the magnify effect below).
  let previewEl = $state<HTMLDivElement | null>(null);
  let canvasWrapperEl = $state<HTMLDivElement | null>(null);

  // SvelteFlow owns the live node/edge arrays so dragging follows the cursor
  // smoothly. The model store stays the source of truth: we sync store → RF on
  // structural/data changes, and write positions back to the store only at drag
  // end (onnodedragstop below).
  let rfNodes = $state<Node[]>([]);
  let rfEdges = $state<Edge[]>([]);

  // useSvelteFlow() (confirmed via hooks/useSvelteFlow.svelte.d.ts) requires flow
  // context — available because Canvas.svelte wraps this component in
  // <SvelteFlowProvider>.
  const { screenToFlowPosition, fitView, flowToScreenPosition, getNodesBounds, getViewport, setViewport } =
    useSvelteFlow();

  // ── Derived ──────────────────────────────────────────────────────────────────
  // Single "focused" element (the sole selected node/edge) for the Inspector; a
  // multi-selection focuses nothing.
  const focused = $derived(focusedSelection(selectionSet));
  // Behavior documents are both model and view — they join the switcher as
  // read-only views alongside curated Diagrams (behavioral substrates spec).
  const behaviorViews = $derived(
    ($model.flows ?? []).map((f): Diagram => ({ key: f.key, title: f.title, profile: "uml-domain", members: [] as string[] })),
  );
  const sequenceViews = $derived(
    ($model.interactions ?? []).map((s): Diagram => ({ key: s.key, title: s.title, profile: "uml-domain", members: [] as string[] })),
  );
  const diagrams = $derived([...effectiveDiagrams($model), ...behaviorViews, ...sequenceViews]);
  const activeFlow = $derived(($model.flows ?? []).find((f) => f.key === activeDiagramKey));
  const activeSequence = $derived(($model.interactions ?? []).find((s) => s.key === activeDiagramKey));
  const activeDiagram = $derived(diagrams.find((d) => d.key === activeDiagramKey) ?? diagrams[0]);
  // The Navigator's scope (which package subtree it shows) and the active
  // profile's create-palette (the metaclasses the context menu offers).
  let scopeKey = $state("");
  const palette = $derived([...getProfile(activeDiagram.profile).palette.metaclasses]);
  // A package's new key after a title rename: keep its parent path, reslug the
  // leaf from the new title.
  const reslugPackage = (key: string, title: string) => {
    const cut = key.lastIndexOf("/");
    return (cut >= 0 ? key.slice(0, cut + 1) : "") + slugify(title);
  };
  // The active diagram's resolved per-diagram render settings (absent ⇒ defaults).
  // Replaces the old global viewMode/relLabelMode browser preferences.
  const activeDisplay = $derived(resolveDisplay(activeDiagram.display));
  const memberSet = $derived(new Set(activeDiagram.members));
  // Element picker entries: the active diagram at the top, then its member
  // objects (nodes), then its associations (edges with both ends in the diagram).
  const nodeTitle = (key: string) =>
    $model.nodes.find((n) => n.key === key)?.concept.title?.trim() || "Untitled";
  const edgeLabel = (e: ModelEdge) =>
    typeof e.name === "string" && e.name.trim()
      ? e.name.trim()
      : `${nodeTitle(e.from)} → ${nodeTitle(e.to)}`;
  const inspectorOptions = $derived([
    {
      key: activeDiagram.key,
      label: activeDiagram.title?.trim() || "Untitled diagram",
      kind: "diagram" as const,
    },
    ...$model.nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => ({ key: n.key, label: n.concept.title?.trim() || "Untitled", kind: "node" as const })),
    ...$model.edges
      .filter((e) => memberSet.has(e.from) && memberSet.has(e.to))
      .map((e) => ({ key: e.id, label: edgeLabel(e), kind: "edge" as const })),
  ]);
  // The picker can also select the active diagram itself, which just shows its
  // name in the panel. That "scope" only applies while no node/edge is focused.
  let inspectorDiagramScope = $state(false);
  const inspectorSelectedKey = $derived(
    focused?.type === "node" || focused?.type === "edge"
      ? focused.id
      : inspectorDiagramScope
        ? activeDiagram.key
        : null,
  );
  const inspectorFocusedKind = $derived(
    focused?.type ?? (inspectorDiagramScope ? "diagram" : undefined),
  );
  const candidateStereotypes = $derived(diagramCandidateStereotypes($model.nodes, activeDiagram.members));
  const diagramEditable = $derived(isDiagramEditable(activeDiagram.key));
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

  // 7) "Magnifying glass": while the central panel is open for an element or
  // edge, pan/zoom the real canvas so the focal node(s) sit behind the panel's
  // transparent preview cutout, instead of rendering a separate cropped
  // preview. Computed once per open (not on every rfNodes change, e.g. a
  // field edit re-rendering the focal node — re-centering on every keystroke
  // would be janky); restores the pre-open viewport on close.
  let savedViewport: Viewport | null = null;
  const MAGNIFY_DURATION = 240;
  $effect(() => {
    const panel = centralPanel;
    if (!panel || panel.kind === "diagram") {
      if (savedViewport) {
        setViewport(savedViewport, { duration: MAGNIFY_DURATION });
        savedViewport = null;
      }
      return;
    }
    if (!previewEl || !canvasWrapperEl) return;
    const preview = previewEl;
    const wrapper = canvasWrapperEl;
    // Everything below reads library internals (getNodesBounds, $model) that
    // Svelte 5 tracks transparently even though we never name them as deps.
    // Left tracked, those reads retrigger this effect on every intermediate
    // measurement/store update during the transition itself, each restart
    // resetting the ease-in-out curve — the animation fights itself and
    // "converges" over several seconds instead of running once. untrack()
    // confines this effect's real dependencies to panel/previewEl/canvasWrapperEl.
    untrack(() => {
      const focalIds =
        panel.kind === "element"
          ? [panel.nodeKey]
          : (() => {
              const focalEdge = $model.edges.find((e) => e.id === panel.edgeKey);
              return focalEdge ? [focalEdge.from, focalEdge.to] : [];
            })();
      if (focalIds.length === 0) return;
      let bounds;
      try {
        bounds = getNodesBounds(focalIds);
      } catch {
        return;
      }
      if (!bounds || !Number.isFinite(bounds.x) || bounds.width <= 0 || bounds.height <= 0) return;
      if (savedViewport === null) savedViewport = getViewport();
      const previewRect = preview.getBoundingClientRect();
      const wrapperRect = wrapper.getBoundingClientRect();
      const PADDING = 0.8; // leaves a margin around the focal bbox inside the cutout
      const zoom = Math.max(
        CANVAS_MIN_ZOOM,
        Math.min(
          CANVAS_MAX_ZOOM,
          Math.min(previewRect.width / bounds.width, previewRect.height / bounds.height) * PADDING,
        ),
      );
      const focalCx = bounds.x + bounds.width / 2;
      const focalCy = bounds.y + bounds.height / 2;
      const targetScreenX = previewRect.left + previewRect.width / 2 - wrapperRect.left;
      const targetScreenY = previewRect.top + previewRect.height / 2 - wrapperRect.top;
      setViewport(
        { x: targetScreenX - focalCx * zoom, y: targetScreenY - focalCy * zoom, zoom },
        { duration: MAGNIFY_DURATION },
      );
    });
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
    // Select the freshly-drawn edge (shows the toolbar and reflects into the
    // always-present Inspector) to set join keys.
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
    // A real canvas selection exits the diagram-name scope.
    if (nodes.length || edges.length) inspectorDiagramScope = false;
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

  // Merge the single-field panel patch onto the current resolved display and
  // persist the full display through the store. On the implicit "All" diagram
  // (no backing doc) store.updateDiagram is a documented no-op.
  function handleDisplayChange(p: Partial<DiagramDisplay>) {
    store.updateDiagram(activeDiagram.key, {
      display: resolveDisplay({ ...activeDiagram.display, ...p }),
    });
  }

  // ── Keyboard delete ────────────────────────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    if (activeFlow || activeSequence) return; // read-only behavior view — never mutate the model
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
    if (activeFlow || activeSequence) return; // read-only behavior view — never mutate the model
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
    if (activeFlow || activeSequence) return; // read-only behavior view — never mutate the model
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

  // Export the canvas as an SVG (whole model, WAML watermark). Uses the live RF
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

  // Replace the whole model with a bundle, then auto-layout it. A fresh model
  // may be purely behavioral (no curated diagram); land on its first real view
  // rather than keeping the previous model's activeDiagramKey. Merge (a
  // different code path) intentionally keeps the user's current view.
  function loadBundleWithLayout(bundle: Bundle) {
    store.load(bundle);
    activeDiagramKey = defaultDiagramKey(store.get());
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
    navOpen={navOpen}
    onToggleNav={() => (navOpen = !navOpen)}
  />

  <CentralEditPanelHost
    state={centralPanel}
    nodes={$model.nodes}
    edges={$model.edges}
    display={activeDisplay}
    diagram={activeDiagram}
    candidateStereotypes={candidateStereotypes}
    editable={diagramEditable}
    profileName={activeDiagram.profile}
    options={inspectorOptions}
    showPreview
    bind:previewEl
    onSelectElement={(key, kind) => {
      if (kind === "diagram") centralPanel = { kind: "diagram" };
      else if (kind === "edge") centralPanel = { kind: "edge", edgeKey: key };
      else centralPanel = { kind: "element", nodeKey: key };
    }}
    onUpdateNode={store.updateNode}
    onUpdateEdge={store.updateEdge}
    onDisplayChange={handleDisplayChange}
    onUpdateDiagram={(patch) => store.updateDiagram(activeDiagram.key, patch)}
    onClose={() => (centralPanel = null)}
  />

  {#if shareToast}
    <ShareToast message={shareToast} onClose={() => (shareToast = null)} />
  {/if}

  {#if showImport}
    <ImportDialog onConfirm={handleImportConfirm} onClose={() => (showImport = false)} />
  {/if}
  {#if showClear}
    <ClearCanvasDialog
      counts={{ nodes: $model.nodes.length, relationships: $model.edges.length }}
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
    <div
      bind:this={canvasWrapperEl}
      class="flex-1 relative {canvasClass}"
      data-canvas-wrapper
      ondblclick={handleWrapperDoubleClick}
      onpointerenter={() => (canvasHovered = true)}
      onpointerleave={() => (canvasHovered = false)}
    >
      <!-- Tool dock — anchored to the canvas (not the outer row) so it sits just
           inside the canvas edge and slides over as the rail opens. The diagram
           switcher now lives in the TopBar title control. -->
      <Dock
        activeTool={tool}
        onToolChange={handleToolChange}
        onClear={() => (showClear = true)}
        clearDisabled={$model.nodes.length === 0}
        onOpenProperties={() => (centralPanel = { kind: "diagram" })}
      />
      {#if activeFlow}
        <FlowView doc={activeFlow} />
      {:else if activeSequence}
        <SequenceView doc={activeSequence} />
      {:else}
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
          minZoom={CANVAS_MIN_ZOOM}
          maxZoom={CANVAS_MAX_ZOOM}
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
          <!-- White out the pane behind the central-edit-panel's magnifying-glass
               cutout so the focal node sits on a clean backdrop instead of the
               normal light-grey working surface. -->
          <Background
            variant={BackgroundVariant.Dots}
            gap={22}
            size={1.3}
            patternColor="#e2e6ec"
            bgColor={centralPanel ? "#ffffff" : undefined}
          />
          <!-- Controls `position` accepts PanelPosition ("bottom-left" etc.),
               confirmed via @xyflow/system dist/esm/types/general.d.ts. The
               feedback link moved to a right-edge flag, so the zoom controls
               return to their normal bottom-left resting position. -->
          <Controls position="bottom-left" style="bottom:15px;left:15px;margin:0;" />
        </SvelteFlow>
      {/if}

      <!-- Empty canvas CTA -->
      {#if $model.nodes.length === 0 && !activeFlow && !activeSequence}
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
           only while ≥1 element is selected AND the pointer is currently
           hovering the canvas (selection alone isn't enough — it would
           otherwise linger after the pointer moves elsewhere, e.g. onto the
           Navigator or Inspector). -->
      {#if !isSelectionEmpty(selectionSet) && toolbarPos && canvasHovered && !activeFlow && !activeSequence}
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

    <!-- Right-edge Feedback flag; anchored to the edge (the Inspector is a
         floating top-right card that doesn't overlap the mid-height flag). -->
    <EdgeFlag
      label="Feedback"
      offset={62}
      href="https://github.com/redoz/waml/issues/new"
    >
      {#snippet icon()}<MessageSquare size={16} />{/snippet}
    </EdgeFlag>

    <!-- Model navigator — two-mode panel (centered modal / left-docked rail).
         Session state lives above; navigation actions close it, structural
         edits leave it open so the user can chain them. -->
    <NavigatorPanel
      open={navOpen}
      mode={navMode}
      bind:width={navWidth}
      bind:collapsed={navCollapsed}
      title={$model.path || "model"}
      onClose={() => (navOpen = false)}
      onToggleMode={() => (navMode = navMode === "centered" ? "docked" : "centered")}
      graph={$model}
      scopeKey={scopeKey}
      activeDiagramKey={activeDiagram.key}
      palette={palette}
      onScope={(key) => (scopeKey = key)}
      onSelectDiagram={(key) => {
        // A selection made in one diagram must never carry into another — most
        // importantly, it must never survive into a read-only Flow/Sequence view
        // (where it would otherwise leave the floating SelectionToolbar's Delete
        // button live against the still-mounted model). See also the
        // activeFlow/activeSequence guards on handleDeleteSelection/handleKeyDown
        // and the <SelectionToolbar> render condition below (defense in depth).
        activeDiagramKey = key;
        selectionSet = EMPTY_SELECTION;
        inspectorDiagramScope = false;
        navOpen = false;
      }}
      onCreateDiagram={(name) => {
        const d = store.addDiagram(name);
        activeDiagramKey = d.key;
        navOpen = false;
      }}
      onReorder={(pkgKey, order) => store.reorderMembers(pkgKey, order)}
      onSort={(pkgKey) => store.sortPackage(pkgKey)}
      onCreatePackage={(parent, name) => store.createGhostPackage(parent, name)}
      onCreateNode={(dir, metaclass) => store.createNodeInPackage(dir, metaclass, metaclass.split(".").pop() || metaclass)}
      onRename={(key, kind, title) => {
        if (kind === "package") store.renamePackage(key, reslugPackage(key, title));
        else if (kind === "diagram") store.updateDiagram(key, { title });
        else {
          const n = $model.nodes.find((x) => x.key === key);
          if (n) store.updateNode(key, { concept: { ...n.concept, title } });
        }
      }}
      onDelete={(key, kind, mode) => {
        if (kind === "package") store.deletePackage(key, mode === "cascade");
        else store.removeNode(key);
      }}
      onViewInDiagram={(key, diagramKey) => {
        activeDiagramKey = diagramKey;
        selectionSet = { nodes: [key], edges: [] };
        navOpen = false;
      }}
      onAddToNewDiagram={(key) => {
        const d = store.addDiagramFromMembers("New diagram", [key]);
        activeDiagramKey = d.key;
        navOpen = false;
      }}
      onEditProperties={(key) => {
        centralPanel = { kind: "element", nodeKey: key };
        navOpen = false;
      }}
    />

    <!-- Always-present floating Inspector (translucent when pinned + idle). -->
    <InspectorPanel
      options={inspectorOptions}
      selectedKey={inspectorSelectedKey}
      focusedKind={inspectorFocusedKind}
      onSelect={(key, kind) => {
        if (kind === "diagram") {
          inspectorDiagramScope = true;
          selectionSet = EMPTY_SELECTION;
        } else if (kind === "edge") {
          inspectorDiagramScope = false;
          selectionSet = key ? { nodes: [], edges: [key] } : EMPTY_SELECTION;
        } else {
          inspectorDiagramScope = false;
          selectionSet = key ? { nodes: [key], edges: [] } : EMPTY_SELECTION;
        }
      }}
      pinned={inspectorPinned}
      bind:width={inspectorWidth}
      onTogglePin={() => (inspectorPinned = !inspectorPinned)}
      onEdit={() => {
        if (inspectorFocusedKind === "diagram") centralPanel = { kind: "diagram" };
        else if (focused?.type === "node") centralPanel = { kind: "element", nodeKey: focused.id };
        else if (focused?.type === "edge") centralPanel = { kind: "edge", edgeKey: focused.id };
      }}
    >
      {#if inspectorFocusedKind === "diagram"}
        <!-- Diagram scope: name only for now; full properties live elsewhere. -->
        <div class="text-[14px] font-semibold text-slate-900">
          {activeDiagram.title?.trim() || "Untitled diagram"}
        </div>
        <div class="mt-1 text-[12px] font-medium uppercase tracking-wide text-slate-400">Diagram</div>
      {:else}
        <InspectorReadonly
          selection={focused}
          nodes={$model.nodes}
          edges={$model.edges}
          onSelectAssociation={(id) => {
            inspectorDiagramScope = false;
            selectionSet = { nodes: [], edges: [id] };
          }}
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
        </InspectorReadonly>
      {/if}
    </InspectorPanel>
  </div>
</div>
