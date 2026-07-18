<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML Actor: stick figure with the name beneath, on Atlas accent strokes. -->
<div class="actor-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <svg class="actor-glyph" width="48" height="72" viewBox="0 0 48 72">
    <circle cx="24" cy="10" r="8" />
    <line x1="24" y1="18" x2="24" y2="44" />
    <line x1="6" y1="28" x2="42" y2="28" />
    <line x1="24" y1="44" x2="8" y2="66" />
    <line x1="24" y1="44" x2="40" y2="66" />
  </svg>
  <div class="actor-name">{data.concept.title ?? data.key}</div>
</div>

<style>
  .actor-node {
    position: relative; display: flex; flex-direction: column; align-items: center;
    width: 120px; cursor: grab; user-select: none;
  }
  .actor-glyph {
    position: relative; z-index: 1;
    fill: none; stroke: rgb(var(--accent)); stroke-width: 2;
    stroke-linecap: round; stroke-linejoin: round;
    filter: drop-shadow(0 0 calc(6px * var(--glow)) rgba(var(--accent), calc(.3 * var(--glow))));
  }
  .actor-glyph circle { fill: var(--panel-fill); }
  .actor-name {
    position: relative; z-index: 1; margin-top: 4px; max-width: 100%; text-align: center;
    font: 700 12px/1.2 var(--font-mono);
    letter-spacing: .06em; text-transform: uppercase; color: var(--ink);
  }
</style>
