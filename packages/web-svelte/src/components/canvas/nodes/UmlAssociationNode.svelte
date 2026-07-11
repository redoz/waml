<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import type { OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!--
  Association class: an ordinary class box (name / attributes), drawn with a dashed
  outline. The dashed connector from the box to the association line's midpoint is
  drawn by the edge renderer (Task 12) for the edge whose `name = { ref }` points here.
-->
<div class="[&>div]:border-dashed"><ClassifierBox {data} keyword="association" /></div>
