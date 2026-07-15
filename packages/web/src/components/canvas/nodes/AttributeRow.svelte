<script lang="ts">
  import type { Attribute } from "@waml/okf";

  // `showTypes` (from the diagram's attributeDetail) toggles ONLY type name.
  // `showMultiplicity` independently toggles trailing `{mult}` suffix.
  let { a, showVisibility, showTypes = true, showMultiplicity = true }:
    { a: Attribute; showVisibility?: boolean; showTypes?: boolean; showMultiplicity?: boolean } = $props();
</script>

<div class="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
  {#if showVisibility && a.visibility}<span class="text-slate-400 font-mono">{a.visibility}</span>{/if}
  <span class="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
  {#if showTypes || showMultiplicity}
    <span class="text-slate-400 font-mono text-[10.5px] truncate">
      {#if showTypes}{a.type.name}{/if}{#if showMultiplicity && a.multiplicity !== "1"} {`{${a.multiplicity}}`}{/if}
    </span>
  {/if}
</div>
