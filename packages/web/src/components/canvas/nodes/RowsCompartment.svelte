<script lang="ts">
  import type { Snippet } from "svelte";
  import { ChevronDown, ChevronRight } from "lucide-svelte";
  import { ERD_COLLAPSED_ROWS } from "@waml/core/canvas/layoutSize";

  // `max` (a diagram authoring cap) overrides the interactive collapse with a
  // static "+K more" footer. Absent -> today's per-box expand/collapse toggle.
  let { rows, max, render }: { rows: number; max?: number; render: Snippet<[number]> } = $props();

  let expanded = $state(false);
</script>

{#if rows > 0}
  {#if max !== undefined}
    {@const visible = Math.min(rows, max)}
    {@const hiddenK = rows - visible}
    <div class="node-comp">
      {#each Array.from({ length: visible }, (_, i) => i) as i (i)}
        {@render render(i)}
      {/each}
      {#if hiddenK > 0}
        <div class="node-more" style="cursor:default">
          +{hiddenK} more
        </div>
      {/if}
    </div>
  {:else}
    {@const visible = expanded ? rows : Math.min(rows, ERD_COLLAPSED_ROWS)}
    {@const hidden = rows - ERD_COLLAPSED_ROWS}
    <div class="node-comp">
      {#each Array.from({ length: visible }, (_, i) => i) as i (i)}
        {@render render(i)}
      {/each}
      {#if hidden > 0}
        <button
          onclick={(e) => { e.stopPropagation(); expanded = !expanded; }}
          class="node-more"
        >
          {#if expanded}
            <ChevronDown size={12} /> Show less
          {:else}
            <ChevronRight size={12} /> +{hidden} more
          {/if}
        </button>
      {/if}
    </div>
  {/if}
{/if}
