<script lang="ts">
  // TODO(plan-3b): port ExternalRefs.tsx here. Props contract is frozen.
  import type { ModelNode, ModelEdge, Diagram } from "@mc/okf";

  let { nodeKey, nodes, edges, members, diagrams, onNavigate }: {
    nodeKey: string;
    nodes: ModelNode[];
    edges: ModelEdge[];
    members: string[];
    diagrams: Diagram[];
    onNavigate: (diagramKey: string, nodeKey: string) => void;
  } = $props();
</script>

<!-- TODO(plan-3b): ExternalRefs markup -->
