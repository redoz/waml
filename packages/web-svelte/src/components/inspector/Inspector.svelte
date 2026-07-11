<script lang="ts">
  // TODO(plan-3b): port Inspector.tsx here. Props contract is frozen.
  import type { Snippet } from "svelte";
  import type { ModelNode, ModelEdge } from "@mc/okf";
  import type { ModelStore } from "@mc/core/state/model";
  import type { Selection } from "../canvas/selection";

  let { selection, nodes, edges, onUpdateNode, onUpdateEdge, onClose, profileName, externalRefs, embedded }: {
    selection: Selection;
    nodes: ModelNode[];
    edges: ModelEdge[];
    onUpdateNode: ModelStore["updateNode"];
    onUpdateEdge: ModelStore["updateEdge"];
    onClose: () => void;
    profileName: string;
    externalRefs?: Snippet;
    embedded?: boolean;
  } = $props();
</script>

<!-- TODO(plan-3b): Inspector markup -->
