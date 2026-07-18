<script lang="ts">
  import type { ActivityNode, FlowFlavor } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const isKeywordOnly = $derived(n.id === n.kind);
</script>

<!-- Control nodes (initial / final / decision-merge / fork-join): solid marker
     glyphs. Their silhouettes are the flow-control shape convention, kept
     literal; all chrome — solid fills route through --ink, borders/strokes
     through rgb(var(--ink-faint)), diamond field through --panel-fill, and the
     caption through --font-mono. -->
<div class="relative flex flex-col items-center select-none">
  <FlowPorts />
  {#if n.kind === "initial"}
    <svg width="36" height="36"><circle cx="18" cy="18" r="10" class="ctl-fill" /></svg>
  {:else if n.kind === "final"}
    <svg width="36" height="36">
      <circle cx="18" cy="18" r="12" fill="none" class="ctl-stroke" stroke-width="2" />
      <circle cx="18" cy="18" r="7" class="ctl-fill" />
    </svg>
  {:else if n.kind === "decision" || n.kind === "merge"}
    <svg width="56" height="56"><path d="M28,4 L52,28 L28,52 L4,28 z" class="ctl-field ctl-stroke" stroke-width="2" /></svg>
  {:else}
    <!-- fork / join: synchronization bar -->
    <div class="ctl-bar"></div>
  {/if}
  {#if !isKeywordOnly}
    <div class="ctl-name">{n.id}</div>
  {/if}
</div>

<style>
  .ctl-fill {
    fill: var(--ink);
  }
  .ctl-field {
    fill: var(--panel-fill);
  }
  .ctl-stroke {
    stroke: rgb(var(--ink-faint));
  }
  .ctl-bar {
    height: 10px;
    width: 120px;
    border-radius: var(--round-chip);
    background: var(--ink);
  }
  .ctl-name {
    margin-top: 4px;
    max-width: 140px;
    text-align: center;
    font: 500 11px/1.2 var(--font-mono);
    color: var(--ink-dim);
  }
</style>
