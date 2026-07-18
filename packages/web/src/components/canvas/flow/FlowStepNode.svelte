<script lang="ts">
  import type { ActivityNode, FlowFlavor } from "@waml/okf";
  import FlowPorts from "./FlowPorts.svelte";

  let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  const n = $derived(data.node);
  const internals = $derived(
    [
      n.entry ? `entry / ${n.entry}` : null,
      n.do ? `do / ${n.do}` : null,
      n.exit ? `exit / ${n.exit}` : null,
    ].filter((x): x is string => x != null),
  );
</script>

<!-- Action (activity) / state (state machine): rounded rect, optional internals.
     The rounded-rect silhouette is the activity/state shape convention (vs the
     object node's plain rect), so its radius is kept literal; all chrome —
     border, fill, ink, hairline, fonts — routes through Atlas tokens. -->
<div class="step-node">
  <FlowPorts />
  <div class="step-name">{n.id}</div>
  {#if internals.length > 0}
    <div class="step-internals">
      {#each internals as row (row)}<div>{row}</div>{/each}
    </div>
  {/if}
  {#if n.refines}
    <div class="step-refines">↳ refines {n.refines}</div>
  {/if}
  {#if n.partition}
    <div class="step-partition">{n.partition}</div>
  {/if}
</div>

<style>
  .step-node {
    position: relative;
    width: 180px;
    padding: 9px 12px;
    text-align: center;
    user-select: none;
    border-radius: 10px;
    border: var(--bw) solid rgb(var(--ink-faint));
    background: var(--panel-fill);
    box-shadow: 0 2px 8px rgba(40, 70, 110, .10);
  }
  .step-name {
    font: 600 12.5px/1.2 var(--font-mono);
    color: var(--ink);
  }
  .step-internals {
    margin-top: 4px;
    padding-top: 4px;
    border-top: 1px solid var(--hair);
    text-align: left;
    font: 500 10.5px/18px var(--font-mono);
    color: var(--ink-dim);
  }
  .step-refines {
    margin-top: 4px;
    text-align: left;
    font: italic 500 10.5px/1.4 var(--font-mono);
    color: rgb(var(--ink-faint));
  }
  .step-partition {
    margin-top: 4px;
    text-align: left;
    font: 500 9.5px/1.4 var(--font-mono);
    letter-spacing: .05em;
    text-transform: uppercase;
    color: rgb(var(--ink-faint));
  }
</style>
