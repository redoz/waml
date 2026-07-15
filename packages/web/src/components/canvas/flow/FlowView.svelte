<script lang="ts">
  import { SvelteFlow, SvelteFlowProvider, Background, BackgroundVariant, Controls, type Edge, type Node } from "@xyflow/svelte";
  import type { FlowDoc } from "@waml/okf";
  import { flowToRf } from "../../../canvas/flowGraph";
  import FlowStepNode from "./FlowStepNode.svelte";
  import FlowControlNode from "./FlowControlNode.svelte";
  import FlowObjectNode from "./FlowObjectNode.svelte";
  import TransitionEdge from "./TransitionEdge.svelte";

  let { doc }: { doc: FlowDoc } = $props();

  const nodeTypes = { flowStep: FlowStepNode, flowControl: FlowControlNode, flowObject: FlowObjectNode };
  const edgeTypes = { transition: TransitionEdge };

  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  $effect(() => {
    const rf = flowToRf(doc);
    nodes = rf.nodes;
    edges = rf.edges;
  });
</script>

<!-- A self-rendering behavior view: read-only, laid out at render time. Its own
     provider keeps this SvelteFlow instance isolated from the structure canvas. -->
<div class="h-full w-full" data-flow-view>
  <SvelteFlowProvider>
    <SvelteFlow bind:nodes bind:edges {nodeTypes} {edgeTypes} fitView nodesDraggable={false} nodesConnectable={false} zoomOnDoubleClick={false} deleteKey={null}>
      <Background variant={BackgroundVariant.Dots} gap={22} size={1.3} patternColor="#e2e6ec" />
      <Controls position="bottom-left" style="bottom:15px;left:15px;margin:0;" />
    </SvelteFlow>
  </SvelteFlowProvider>
</div>
