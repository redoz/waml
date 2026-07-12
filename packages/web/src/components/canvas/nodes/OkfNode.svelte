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
  <div class="relative rounded-full border border-[#d8dee8] bg-white px-3 py-[6px] text-[12px] font-[600] text-slate-600 shadow-sm">
    <NodePorts />
    <span class="relative z-[1]">{node.concept.title ?? "Untitled"}</span>
  </div>
{:else}
  {@const Renderer = resolveNodeRenderer(node.type)}
  <Renderer data={node} />
{/if}
