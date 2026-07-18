<script lang="ts">
  import { BaseEdge, EdgeLabel, getBezierPath, getSmoothStepPath, useInternalNode, type EdgeProps } from "@xyflow/svelte";
  import { decisionSourceTip, getEdgeParams, type NodeGeom } from "../floating";

  let { id, source, target, data, selected }: EdgeProps = $props();

  const sourceInternal = $derived(useInternalNode(source));
  const targetInternal = $derived(useInternalNode(target));
  const sourceNode = $derived(sourceInternal.current as NodeGeom | undefined);
  const targetNode = $derived(targetInternal.current as NodeGeom | undefined);
  const d = $derived(data as { label?: string; carries?: string; flavor?: string; fromKind?: string } | undefined);

  // Floating border intersection for both ends, then — when the source is a
  // decision diamond — snap its attach point to the tip facing the target.
  const geometry = $derived.by(() => {
    if (!sourceNode || !targetNode) return undefined;
    const p = getEdgeParams(sourceNode, targetNode);
    if (d?.fromKind === "decision") {
      const tip = decisionSourceTip(sourceNode, targetNode);
      return { ...p, sx: tip.x, sy: tip.y, sourcePos: tip.pos };
    }
    return p;
  });

  const edgePath = $derived.by(() => {
    if (!geometry) return undefined;
    // Activity diagrams read as curved splines; state machines keep smooth-step.
    const params = {
      sourceX: geometry.sx,
      sourceY: geometry.sy,
      sourcePosition: geometry.sourcePos,
      targetX: geometry.tx,
      targetY: geometry.ty,
      targetPosition: geometry.targetPos,
    };
    const [p] = d?.flavor === "activity" ? getBezierPath(params) : getSmoothStepPath({ ...params, borderRadius: 8 });
    return p;
  });

  const stroke = $derived(selected ? "rgb(var(--accent))" : "rgb(var(--ink-faint))");
  const strokeWidth = $derived(selected ? 2.5 : 1.6);
  const edgeStyle = $derived(
    `stroke:${stroke};stroke-width:${strokeWidth};${selected ? "filter:drop-shadow(0 0 2.5px rgba(var(--accent),.35));" : ""}`,
  );
</script>

{#if edgePath && geometry}
  <defs>
    <marker id="flow-arrow-{id}" markerWidth="12" markerHeight="12" refX="10" refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,1 L10,6 L1,11" fill="none" stroke={stroke} stroke-width="1.5" />
    </marker>
  </defs>
  <BaseEdge {id} path={edgePath} markerEnd="url(#flow-arrow-{id})" style={edgeStyle} />
  {#if d?.label}
    <EdgeLabel
      x={(geometry.sx + geometry.tx) / 2}
      y={(geometry.sy + geometry.ty) / 2 - 10}
      class="nodrag nopan"
      style="background:linear-gradient(180deg,rgba(255,255,255,.95),rgba(255,255,255,.82));border-radius:0;padding:1px 5px;font-family:var(--font-mono);font-size:10.5px;font-weight:600;letter-spacing:.04em;color:var(--ink);box-shadow:0 0 0 1px rgba(var(--accent),.22);white-space:nowrap;"
    >
      {d.label}
    </EdgeLabel>
  {/if}
{/if}
