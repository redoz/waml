<script lang="ts">
  import ClassifierBox from "./ClassifierBox.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- Tabbed-folder: a small frosted tab above the ClassifierBox body. -->
<div class="pkg-wrap" style={`--accent:${hexToTriple()}`}>
  <div class="pkg-tab"></div>
  <ClassifierBox {data} />
</div>

<style>
  .pkg-wrap { position: relative; }
  .pkg-tab {
    position: absolute; top: -10px; left: 10px; height: 12px; width: 64px;
    border-radius: var(--round) var(--round) 0 0;
    border: var(--bw) solid rgba(var(--accent), .5); border-bottom: 0;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, .94), rgba(255, 255, 255, .80)),
      rgba(var(--accent), .06);
  }
</style>
