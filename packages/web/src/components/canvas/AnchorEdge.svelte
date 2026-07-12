<script lang="ts">
  // Mirrors packages/web/src/components/canvas/AnchorEdge.tsx.
  import { BaseEdge, getStraightPath, useInternalNode, type EdgeProps } from "@xyflow/svelte";
  import { getEdgeParams, type NodeGeom } from "./floating";

  // useInternalNode(id) accessor shape confirmed via
  // node_modules/@xyflow/svelte/dist/lib/hooks/useInternalNode.svelte.d.ts:
  //   export declare function useInternalNode(id: string): { current: InternalNode | undefined };
  let { id, source, target }: EdgeProps = $props();

  // Derive the accessor once (re-created only on source/target change), read
  // `.current` downstream — see RelEdge.svelte for why the collapsed form froze
  // edge geometry until drag stop.
  const sourceInternal = $derived(useInternalNode(source));
  const targetInternal = $derived(useInternalNode(target));
  const sourceNode = $derived(sourceInternal.current as NodeGeom | undefined);
  const targetNode = $derived(targetInternal.current as NodeGeom | undefined);

  // Floating endpoints, but the dashed connector stays a straight line.
  const path = $derived.by(() => {
    if (!sourceNode || !targetNode) return undefined;
    const { sx, sy, tx, ty } = getEdgeParams(sourceNode, targetNode);
    const [p] = getStraightPath({ sourceX: sx, sourceY: sy, targetX: tx, targetY: ty });
    return p;
  });
</script>

{#if sourceNode && targetNode && path}
  <BaseEdge {id} {path} style="stroke:#94a3b8;stroke-width:1.2;stroke-dasharray:4 3;" />
{/if}
