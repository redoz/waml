<script lang="ts">
  import type { ActivityNode, FlowFlavor } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
</script>

<!-- Object/data node: plain rectangle (no radius) — the sharp-cornered
     silhouette is the data-object shape convention (vs the activity/state
     node's rounded rect), so it is kept literal; all chrome — border, fill,
     ink, fonts — routes through Atlas tokens. -->
<div class="object-node">
  <FlowPorts />
  <div class="object-name" class:underline={!!n.objectRef}>{n.id}</div>
</div>

<style>
  .object-node {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 160px;
    height: 48px;
    padding: 0 12px;
    text-align: center;
    user-select: none;
    border: var(--bw) solid rgb(var(--ink-faint));
    background: var(--panel-fill);
  }
  .object-name {
    font: 600 12px/1.2 var(--font-mono);
    color: var(--ink);
  }
  .underline {
    text-decoration: underline;
  }
</style>
