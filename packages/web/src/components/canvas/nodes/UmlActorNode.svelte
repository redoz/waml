<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML Actor: stick figure name beneath (uaml behavioral-substrates spec). -->
<div class="relative flex w-[120px] flex-col items-center select-none">
  <NodePorts />
  <svg width="48" height="72" viewBox="0 0 48 72" class="relative z-[1]">
    <circle cx="24" cy="10" r="8" fill="#fff" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="18" x2="24" y2="44" stroke="#334155" stroke-width="2" />
    <line x1="6" y1="28" x2="42" y2="28" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="44" x2="8" y2="66" stroke="#334155" stroke-width="2" />
    <line x1="24" y1="44" x2="40" y2="66" stroke="#334155" stroke-width="2" />
  </svg>
  <div class="relative z-[1] mt-1 max-w-full text-center text-[12px] font-semibold text-slate-800">
    {data.concept.title ?? data.key}
  </div>
</div>
