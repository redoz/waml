<script lang="ts">
  import type { Attribute } from "@waml/okf";

  // `showTypes` (from the diagram's showType) toggles ONLY type name.
  // `showMultiplicity` independently toggles trailing `{mult}` suffix.
  let { a, showVisibility, showTypes = true, showMultiplicity = true }:
    { a: Attribute; showVisibility?: boolean; showTypes?: boolean; showMultiplicity?: boolean } = $props();
</script>

<div class="node-row">
  {#if showVisibility && a.visibility}<span class="node-row__vis">{a.visibility}</span>{/if}
  <span class="node-row__name" title={a.name}>{a.name}</span>
  {#if showTypes || showMultiplicity}
    <span class="node-row__ty">
      {#if showTypes}: {a.type.name}{/if}{#if showMultiplicity && a.multiplicity !== "1"} {`{${a.multiplicity}}`}{/if}
    </span>
  {/if}
</div>
