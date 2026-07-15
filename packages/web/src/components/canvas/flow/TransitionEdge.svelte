<script lang="ts">
  import { BaseEdge, EdgeLabel, getSmoothStepPath, useInternalNode, type EdgeProps } from "@xyflow/svelte";
  import { getEdgeParams, type NodeGeom } from "../floating";

  let { id, source, target, data }: EdgeProps = $props();

  const sourceInternal = $derived(useInternalNode(source));
  const targetInternal = $derived(useInternalNode(target));
  const sourceNode = $derived(sourceInternal.current as NodeGeom | undefined);
  const targetNode = $derived(targetInternal.current as NodeGeom | undefined);
  const geometry = $derived(sourceNode && targetNode ? getEdgeParams(sourceNode, targetNode) : undefined);
  const d = $derived(data as { label?: string; carries?: string } | undefined);

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
</script>

{#if edgePath && geometry}
  <defs>
    <marker id="flow-arrow-{id}" markerWidth="12" markerHeight="12" refX="10" refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,1 L10,6 L1,11" fill="none" stroke="#334155" stroke-width="1.5" />
    </marker>
  </defs>
  <BaseEdge {id} path={edgePath} markerEnd="url(#flow-arrow-{id})" style="stroke:#334155;stroke-width:1.6;" />
  {#if d?.label}
    <EdgeLabel
      x={(geometry.sx + geometry.tx) / 2}
      y={(geometry.sy + geometry.ty) / 2 - 10}
      class="nodrag nopan"
      style="background:rgba(255,255,255,0.9);border-radius:4px;padding:0 4px;font-size:10.5px;font-weight:600;color:#334155;white-space:nowrap;"
    >
      {d.label}
    </EdgeLabel>
  {/if}
{/if}
