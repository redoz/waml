<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<ClassifierBox {data} keyword="dataType" />
