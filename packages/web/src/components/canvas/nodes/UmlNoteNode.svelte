<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML Comment: a dog-eared note box carrying the markdown body; NO attribute /
     operation compartments. Its dashed anchor(s) to the annotated element(s) are
     drawn by the edge/anchor layer (Task 12). -->
<div
  class="relative w-[210px] bg-[#fffdf3] border-[1.5px] border-[#e3d9a8] shadow-[0_2px_8px_rgba(15,23,42,0.05)] select-none"
  style="clip-path: polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 0 100%);"
>
  <NodePorts />
  <div class="absolute top-0 right-0 z-[1] h-[14px] w-[14px] border-l border-b border-[#e3d9a8] bg-[#f3ebc0]"></div>
  <div class="relative z-[1] px-3 py-[9px] text-[11.5px] leading-snug text-slate-700 whitespace-pre-wrap">
    {data.body ?? data.title}
  </div>
</div>
