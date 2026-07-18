<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML Comment: a dog-eared note carrying the markdown body. clip-path fights
     .hud-surface's masked ::before frame + box-shadow glow, so this uses a bespoke
     two-layer clip-path frame + drop-shadow glow instead. NO attribute/operation
     compartments; dashed anchors are drawn by the edge/anchor layer. -->
<div class="note-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <div class="note-body">
    {data.note_body ?? data.concept.title}
  </div>
</div>

<style>
  .note-node {
    --fold: 14px;
    position: relative; width: 210px; cursor: grab; user-select: none;
    /* accent layer = the source-bright frame, shows through the inner inset */
    background: linear-gradient(150deg, rgba(var(--accent), .95), rgba(var(--accent), .5));
    clip-path: polygon(0 0, calc(100% - var(--fold)) 0, 100% var(--fold), 100% 100%, 0 100%);
    filter:
      drop-shadow(0 6px 12px rgba(40, 70, 110, .14))
      drop-shadow(0 0 calc(10px * var(--glow)) rgba(var(--accent), calc(.5 * var(--glow))));
  }
  /* inner frost sheet, inset by the border weight, same folded silhouette */
  .note-node::before {
    content: ""; position: absolute; inset: var(--bw);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, .94), rgba(255, 255, 255, .80)),
      rgba(var(--accent), .06);
    clip-path: polygon(0 0, calc(100% - var(--fold)) 0, 100% var(--fold), 100% 100%, 0 100%);
    pointer-events: none;
  }
  /* the dog-ear crease */
  .note-node::after {
    content: ""; position: absolute; top: 0; right: 0; z-index: 1;
    width: var(--fold); height: var(--fold);
    background: rgba(var(--accent), .18);
    clip-path: polygon(0 0, 100% 100%, 0 100%);
    pointer-events: none;
  }
  .note-body {
    position: relative; z-index: 1; padding: 9px 12px;
    font: 500 11.5px/1.5 var(--font-mono); color: var(--ink-dim);
    white-space: pre-wrap;
  }
</style>
