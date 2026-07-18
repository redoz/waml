<script lang="ts">
  import type { NodeProps } from "@xyflow/svelte";
  import NodePorts from "./NodePorts.svelte";
  import { resolveNodeRenderer } from "./registry";
  import type { OkfNodeData } from "./types";

  let { data }: NodeProps = $props();
  let node = $derived(data as unknown as OkfNodeData);
</script>

{#if node._collapsed}
  <!-- A collapsed diagram member renders as a compact ref chip (a "drawn as ref chip"
       hint), keeping off-focus classifiers present but small. -->
  <div class="ref-chip">
    <NodePorts />
    <span class="ref-chip__label">{node.concept.title ?? "Untitled"}</span>
  </div>
{:else}
  {@const Renderer = resolveNodeRenderer(node.type)}
  <Renderer data={node} />
{/if}

<style>
  /* Collapsed off-focus classifier: a faint compact ref chip. */
  .ref-chip {
    position: relative;
    padding: 6px 12px;
    border-radius: var(--round-chip);
    border: 1px solid rgb(var(--ink-faint));
    background: var(--panel-fill);
    box-shadow: 0 1px 3px rgba(40, 70, 110, .12);
    font: 600 12px/1 var(--font-mono);
    color: var(--ink-dim);
  }
  .ref-chip__label { position: relative; z-index: 1; }
</style>
