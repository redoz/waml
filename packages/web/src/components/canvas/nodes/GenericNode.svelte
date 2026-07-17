<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<ClassifierBox {data}>
  {#snippet header()}
    <div class="px-3 pt-[8px]">
      <span class="text-[10px] font-[700] uppercase tracking-[0.3px] px-[7px] py-[2px] rounded-full text-white bg-[#94a3b8]">
        {data.type}
      </span>
    </div>
  {/snippet}
</ClassifierBox>
