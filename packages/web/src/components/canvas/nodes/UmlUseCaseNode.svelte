<script lang="ts">
  import NodePorts from "./NodePorts.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data }: { data: OkfNodeData } = $props();
</script>

<!-- UML UseCase: ellipse with the name centered, on Atlas node material. -->
<div class="hud-surface hud-surface--node uc-node" style={`--accent:${hexToTriple()}`}>
  <NodePorts />
  <div class="uc-name">{data.concept.title ?? data.key}</div>
</div>

<style>
  .uc-node {
    width: 190px; height: 72px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    padding: 0 20px; cursor: grab; user-select: none;
  }
  .uc-name {
    position: relative; z-index: 1; text-align: center;
    font: 700 12.5px/1.2 var(--font-mono);
    letter-spacing: .06em; text-transform: uppercase; color: var(--ink);
  }
</style>
