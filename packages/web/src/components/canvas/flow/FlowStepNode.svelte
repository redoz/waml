<script lang="ts">
  import type { ActivityNode, FlowFlavor } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const internals = $derived(
    [
      n.entry ? `entry / ${n.entry}` : null,
      n.do ? `do / ${n.do}` : null,
      n.exit ? `exit / ${n.exit}` : null,
    ].filter((x): x is string => x != null),
  );
</script>

<!-- Action (activity) / state (state machine): rounded rect, optional internals. -->
<div
  class="relative w-[180px] select-none rounded-[12px] border-[1.5px] border-[#c8d2e0] bg-white px-3 py-[9px] text-center shadow-[0_2px_8px_rgba(15,23,42,0.05)]"
>
  <FlowPorts />
  <div class="text-[12.5px] font-semibold text-slate-800">{n.id}</div>
  {#if internals.length > 0}
    <div class="mt-1 border-t border-[#e2e8f0] pt-1 text-left text-[10.5px] leading-[18px] text-slate-600">
      {#each internals as row (row)}<div>{row}</div>{/each}
    </div>
  {/if}
  {#if n.refines}
    <div class="mt-1 text-left text-[10.5px] italic text-slate-500">↳ refines {n.refines}</div>
  {/if}
  {#if n.partition}
    <div class="mt-1 text-left text-[9.5px] uppercase tracking-wide text-slate-400">{n.partition}</div>
  {/if}
</div>
