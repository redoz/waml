<script lang="ts">
  import type { ActivityNode, FlowFlavor } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const isKeywordOnly = $derived(n.id === n.kind);
</script>

<div class="relative flex flex-col items-center select-none">
  <FlowPorts />
  {#if n.kind === "initial"}
    <svg width="36" height="36"><circle cx="18" cy="18" r="10" fill="#334155" /></svg>
  {:else if n.kind === "final"}
    <svg width="36" height="36">
      <circle cx="18" cy="18" r="12" fill="none" stroke="#334155" stroke-width="2" />
      <circle cx="18" cy="18" r="7" fill="#334155" />
    </svg>
  {:else if n.kind === "decision" || n.kind === "merge"}
    <svg width="56" height="56"><path d="M28,4 L52,28 L28,52 L4,28 z" fill="#fff" stroke="#334155" stroke-width="2" /></svg>
  {:else}
    <!-- fork / join: synchronization bar -->
    <div class="h-[10px] w-[120px] rounded-[2px] bg-[#334155]"></div>
  {/if}
  {#if !isKeywordOnly}
    <div class="mt-1 max-w-[140px] text-center text-[11px] font-medium text-slate-700">{n.id}</div>
  {/if}
</div>
