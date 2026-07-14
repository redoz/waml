<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML UseCase: ellipse with the name centered. -->
<div
  class="relative flex h-[72px] w-[190px] items-center justify-center border-[1.5px] border-[#c8d2e0] bg-white px-4 text-center shadow-[0_2px_8px_rgba(15,23,42,0.05)] select-none"
  style="border-radius: 50%;"
>
  <NodePorts />
  <div class="relative z-[1] text-[12.5px] font-semibold leading-snug text-slate-800">
    {data.concept.title ?? data.key}
  </div>
</div>
