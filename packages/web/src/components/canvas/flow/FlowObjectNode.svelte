<script lang="ts">
  import type { FlowFlavor, FlowNode } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
</script>

<!-- Object/data node: plain rectangle, typed by a link when resolved. -->
<div
  class="relative flex h-[48px] w-[160px] items-center justify-center border-[1.5px] border-[#c8d2e0] bg-[#f8fafc] px-3 text-center select-none"
>
  <FlowPorts />
  <div class="text-[12px] font-semibold text-slate-800" class:underline={!!n.objectRef}>{n.id}</div>
</div>
