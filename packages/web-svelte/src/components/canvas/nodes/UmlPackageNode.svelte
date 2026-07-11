<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- Tabbed-folder: a small tab above the box. -->
<div class="relative">
  <div class="absolute -top-[10px] left-[10px] h-[12px] w-[64px] rounded-t-md border-[1.5px] border-b-0 border-[#d8dee8] bg-white"></div>
  <ClassifierBox {data} />
</div>
