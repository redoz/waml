<script lang="ts">
  import type { Attribute } from "@waml/okf";

  // `showTypes` (from the diagram's attributeDetail) toggles the trailing
  // type/multiplicity column; "name-only" hides it, "name-type" shows it.
  let { a, showVisibility, showTypes = true }: { a: Attribute; showVisibility?: boolean; showTypes?: boolean } = $props();
</script>

<div class="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
  {#if showVisibility && a.visibility}<span class="text-slate-400 font-mono">{a.visibility}</span>{/if}
  <span class="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
  {#if showTypes}
    <span class="text-slate-400 font-mono text-[10.5px] truncate">
      {a.type.name}{a.multiplicity !== "1" ? ` {${a.multiplicity}}` : ""}
    </span>
  {/if}
</div>
