<!-- packages/web/src/components/inspector/InspectorReadonly.svelte -->
<script lang="ts">
  // Read-only docked-panel body. Mirrors Inspector.svelte's embedded dispatch,
  // but shows static field summaries instead of editable inputs — editing moves
  // to the CentralEditPanel dialog (opened via the panel's Edit button).
  import type { Snippet } from "svelte";
  import type { ModelNode, ModelEdge } from "@waml/okf";
  import type { Selection } from "../canvas/selection";
  import ObjectInspectorReadonly from "./ObjectInspectorReadonly.svelte";
  import RelationshipInspectorReadonly from "./RelationshipInspectorReadonly.svelte";

  let { selection, nodes, edges, externalRefs }: {
    selection: Selection;
    nodes: ModelNode[];
    edges: ModelEdge[];
    externalRefs?: Snippet;
  } = $props();

  const selectedNode = $derived(
    selection?.type === "node" ? nodes.find((n) => n.key === selection.id) : undefined,
  );
  const selectedEdge = $derived(
    selection?.type === "edge" ? edges.find((e) => e.id === selection.id) : undefined,
  );
</script>

{#if selectedNode}
  <ObjectInspectorReadonly node={selectedNode} {nodes} {edges} />
  {@render externalRefs?.()}
{:else if selectedEdge}
  <RelationshipInspectorReadonly
    edge={selectedEdge}
    fromNode={nodes.find((n) => n.key === selectedEdge.from)}
    toNode={nodes.find((n) => n.key === selectedEdge.to)}
  />
{/if}
