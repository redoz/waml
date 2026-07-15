<script lang="ts">
  // Mirrors packages/web/src/components/canvas/RelEdge.tsx.
  import { BaseEdge, EdgeLabel, EdgeReconnectAnchor, getSmoothStepPath, useInternalNode, type EdgeProps, type Position } from "@xyflow/svelte";
  import type { ModelEdge, RelEnd, RelationshipKind, DiagramDisplay } from "@waml/okf";
  import { getEdgeParams, portPoint, type NodeGeom, type Rect, type Slot } from "./floating";

  type RelEdgeData = Pick<ModelEdge, "kind" | "fromEnd" | "toEnd" | "bidirectional"> & {
    associationLabels?: DiagramDisplay["associationLabels"];
    modelEdgeId?: string;
    emphasizeMultiplicity?: boolean;
    // Pre-assigned by edges.ts so a hub's edges space themselves along each border.
    sourceSide?: Position;
    targetSide?: Position;
    sourceSlot?: Slot;
    targetSlot?: Slot;
  };

  const rectOf = (n: NodeGeom): Rect => ({
    x: n.internals.positionAbsolute.x,
    y: n.internals.positionAbsolute.y,
    w: n.measured?.width ?? 0,
    h: n.measured?.height ?? 0,
  });

  const DASHED: ReadonlySet<RelationshipKind> = new Set(["implements", "depends"]);

  // useInternalNode(id) accessor shape confirmed via
  // node_modules/@xyflow/svelte/dist/lib/hooks/useInternalNode.svelte.d.ts:
  //   export declare function useInternalNode(id: string): { current: InternalNode | undefined };
  let { id, source, target, data, selected, style }: EdgeProps = $props();

  // Derive the accessor object once (re-created only when source/target change),
  // then read `.current` reactively downstream — mirrors @xyflow/svelte's own
  // MinimapNode. Calling useInternalNode() *inside* the .current derived orphaned
  // its internal subscription to the `nodes` signal, so the edge only re-tracked
  // node positions on a full re-render (drag stop) instead of live during a drag.
  const sourceInternal = $derived(useInternalNode(source));
  const targetInternal = $derived(useInternalNode(target));
  const sourceNode = $derived(sourceInternal.current as NodeGeom | undefined);
  const targetNode = $derived(targetInternal.current as NodeGeom | undefined);

  const d = $derived(data as unknown as RelEdgeData | undefined);
  const kind = $derived<RelationshipKind>(d?.kind ?? "associates");
  const fromEnd = $derived<RelEnd>(d?.fromEnd ?? {});
  const toEnd = $derived<RelEnd>(d?.toEnd ?? {});
  const mode = $derived<DiagramDisplay["associationLabels"]>(d?.associationLabels ?? "all");

  // Floating endpoints. When edges.ts has assigned a side + slot (and both nodes
  // are measured), place the point on that border spaced by the slot so a hub's
  // edges fan out; otherwise fall back to the plain geometric border intersection.
  const geometry = $derived.by(() => {
    if (!sourceNode || !targetNode) return undefined;
    const measured =
      !!sourceNode.measured?.width && !!sourceNode.measured?.height && !!targetNode.measured?.width && !!targetNode.measured?.height;
    if (measured && d?.sourceSide && d?.targetSide) {
      const sp = portPoint(rectOf(sourceNode), d.sourceSide, d.sourceSlot);
      const tp = portPoint(rectOf(targetNode), d.targetSide, d.targetSlot);
      return { sx: sp.x, sy: sp.y, tx: tp.x, ty: tp.y, sourcePos: d.sourceSide, targetPos: d.targetSide };
    }
    return getEdgeParams(sourceNode, targetNode);
  });

  const edgePath = $derived.by(() => {
    if (!geometry) return undefined;
    const [p] = getSmoothStepPath({
      sourceX: geometry.sx,
      sourceY: geometry.sy,
      sourcePosition: geometry.sourcePos,
      targetX: geometry.tx,
      targetY: geometry.ty,
      targetPosition: geometry.targetPos,
      borderRadius: 8,
    });
    return p;
  });

  const stroke = $derived(selected ? "#1e88e5" : "#64748b");
  const strokeWidth = $derived(selected ? 2.5 : 1.8);
  const edgeStyle = $derived(
    `stroke:${stroke};stroke-width:${strokeWidth};${DASHED.has(kind) ? "stroke-dasharray:6 4;" : ""}${style ?? ""}`,
  );

  // Verb → end adornments (spec table).
  type MarkerDef =
    | { type: "diamond"; mid: "diamond-filled" | "diamond-hollow"; fill: string }
    | { type: "triangle" }
    | { type: "arrow"; key: "dep-arrow" | "nav-end" | "nav-start"; flip: boolean };

  const markerInfo = $derived.by(() => {
    let markerStart: string | undefined;
    let markerEnd: string | undefined;
    const defs: MarkerDef[] = [];

    if (kind === "composes") {
      defs.push({ type: "diamond", mid: "diamond-filled", fill: stroke });
      markerStart = `url(#diamond-filled-${id})`;
    } else if (kind === "aggregates") {
      defs.push({ type: "diamond", mid: "diamond-hollow", fill: "#fff" });
      markerStart = `url(#diamond-hollow-${id})`;
    } else if (kind === "specializes" || kind === "implements") {
      defs.push({ type: "triangle" });
      markerEnd = `url(#triangle-${id})`;
    } else if (kind === "depends") {
      defs.push({ type: "arrow", key: "dep-arrow", flip: false });
      markerEnd = `url(#dep-arrow-${id})`;
    } else {
      // associates: arrowhead on navigable end(s)
      if (toEnd.navigable) {
        defs.push({ type: "arrow", key: "nav-end", flip: false });
        markerEnd = `url(#nav-end-${id})`;
      }
      if (fromEnd.navigable) {
        defs.push({ type: "arrow", key: "nav-start", flip: true });
        markerStart = `url(#nav-start-${id})`;
      }
    }
    return { markerStart, markerEnd, defs };
  });

  const endText = (e: RelEnd) => [e.multiplicity, e.role].filter(Boolean).join(" ");
  // `associationLabels` alone decides whether labels show; `emphasizeMultiplicity`
  // is a separate visual emphasis (bolder/larger multiplicity text) applied to them.
  const showLabels = $derived(mode !== "hidden");
  const emphasize = $derived(d?.emphasizeMultiplicity ?? false);
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

  const labels = $derived.by(() => {
    const out: { x: number; y: number; text: string }[] = [];
    if (!showLabels || !geometry) return out;
    const { sx, sy, tx, ty } = geometry;
    const ft = endText(fromEnd);
    const tt = endText(toEnd);
    if (ft) out.push({ x: lerp(sx, tx, 0.18), y: lerp(sy, ty, 0.18) - 10, text: ft });
    if (tt) out.push({ x: lerp(sx, tx, 0.82), y: lerp(sy, ty, 0.82) - 10, text: tt });
    return out;
  });
</script>

{#if sourceNode && targetNode && edgePath}
  <defs>
    {#each markerInfo.defs as m, i (i)}
      {#if m.type === "diamond"}
        <marker id="{m.mid}-{id}" markerWidth="14" markerHeight="10" refX="1" refY="5" orient="auto" markerUnits="userSpaceOnUse">
          <path d="M1,5 L7,1 L13,5 L7,9 z" fill={m.fill} stroke={stroke} stroke-width="1" />
        </marker>
      {:else if m.type === "triangle"}
        <marker id="triangle-{id}" markerWidth="14" markerHeight="12" refX="12" refY="6" orient="auto" markerUnits="userSpaceOnUse">
          <path d="M1,1 L12,6 L1,11 z" fill="#fff" stroke={stroke} stroke-width="1.2" />
        </marker>
      {:else if m.type === "arrow"}
        <marker
          id="{m.key}-{id}"
          markerWidth="12"
          markerHeight="12"
          refX={m.flip ? 1 : 10}
          refY="6"
          orient="auto"
          markerUnits="userSpaceOnUse"
        >
          <path d={m.flip ? "M10,1 L1,6 L10,11" : "M1,1 L10,6 L1,11"} fill="none" stroke={stroke} stroke-width="1.5" />
        </marker>
      {/if}
    {/each}
  </defs>
  <BaseEdge {id} path={edgePath} markerStart={markerInfo.markerStart} markerEnd={markerInfo.markerEnd} style={edgeStyle} />
  {#if selected && geometry}
    <!-- Drag either endpoint to rewire this relationship onto a different node
         (SvelteFlow's v1 reconnect mechanism — no React-Flow-style `reconnectable`
         edge/flow flags exist here, so scope to selected via this local gate). -->
    <EdgeReconnectAnchor type="source" position={{ x: geometry.sx, y: geometry.sy }} />
    <EdgeReconnectAnchor type="target" position={{ x: geometry.tx, y: geometry.ty }} />
  {/if}
  {#each labels as l, i (i)}
    <EdgeLabel
      x={l.x}
      y={l.y}
      class="nodrag nopan"
      style="background:rgba(255,255,255,0.9);border-radius:4px;padding:0 4px;font-size:{emphasize
        ? 12
        : 10.5}px;font-weight:{emphasize ? 800 : 600};color:{emphasize ? '#0f172a' : '#334155'};white-space:nowrap;"
    >
      {l.text}
    </EdgeLabel>
  {/each}
{/if}
