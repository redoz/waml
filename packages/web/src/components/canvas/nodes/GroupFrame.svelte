<script lang="ts">
  // A group hull for a `with frame` layout group: a titled, dashed bordered box
  // sized to the solver's rect. Only `shape === "Frame"` groups reach this
  // renderer (Box/Shrink shape the layout but draw nothing). It is a
  // non-interactive backdrop — selectable/draggable/deletable are set false on
  // the pseudo-node (see toGroupNode), so pointer events pass through.
  // Props are narrowed to the group data this renderer reads; SvelteFlow injects
  // the rest of NodeProps at runtime but only `data` is consumed here.
  let { data: group }: { data: { title?: string; width: number; height: number } } = $props();
</script>

<div
  data-group-frame
  class="group-frame pointer-events-none relative h-full w-full"
  style={`width:${group.width}px;height:${group.height}px;`}
>
  {#if group.title}
    <div data-group-frame-title class="group-frame__title absolute -top-[10px] left-3 px-2">
      {group.title}
    </div>
  {/if}
</div>

<style>
  .group-frame {
    border-radius: var(--round);
    border: var(--bw) dashed rgba(var(--ink-faint), 0.5);
    background: rgba(var(--ink-faint), 0.05);
  }
  .group-frame__title {
    background: var(--canvas-bg);
    font: 600 12px/1.3 var(--font-ui);
    color: rgb(var(--ink-faint));
  }
</style>
