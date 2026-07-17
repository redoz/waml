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
  class="pointer-events-none relative h-full w-full rounded-lg border-2 border-dashed border-slate-300 bg-slate-50/40"
  style={`width:${group.width}px;height:${group.height}px;`}
>
  {#if group.title}
    <div
      data-group-frame-title
      class="absolute -top-[10px] left-3 bg-[#f7f8fa] px-2 text-[12px] font-semibold text-slate-500"
    >
      {group.title}
    </div>
  {/if}
</div>
