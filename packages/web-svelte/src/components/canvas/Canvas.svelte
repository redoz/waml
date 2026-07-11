<script lang="ts">
  import { SvelteFlow, Background, Controls } from "@xyflow/svelte";
  import type { Node, Edge } from "@xyflow/svelte";

  // SvelteFlow owns the live node/edge arrays (smooth dragging). Empty for the
  // blank shell — Plan 3a syncs these from the model store (store → arrays on
  // structural change, positions written back on drag end).
  //
  // Plan 3a note: `useSvelteFlow()` (for screenToFlowPosition/fitView) must be
  // called from inside a <SvelteFlowProvider>. When that is added, wrap this
  // component's <SvelteFlow> in <SvelteFlowProvider> (mirrors React's
  // <ReactFlowProvider> around <CanvasInner>).
  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
</script>

<div class="h-screen w-screen">
  <SvelteFlow bind:nodes bind:edges fitView>
    <Background gap={22} />
    <Controls />
  </SvelteFlow>
</div>
