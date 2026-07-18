<script lang="ts">
  // Private helper for LibraryDialog.svelte — mirrors the NodeRow() function
  // in packages/web/src/components/LibraryDialog.tsx. Split into its own
  // component because it owns its own `open` toggle state, which a snippet
  // can't hold cleanly.
  import { ChevronRight, ChevronDown } from "lucide-svelte";
  import { DataNodeIcon } from "../lib/icons";

  let { title, fields }: {
    title: string;
    fields: { name: string; type: { name: string } }[];
  } = $props();

  let open = $state(false);
</script>

<div class="rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-[color:var(--panel-fill)]">
  <button onclick={() => (open = !open)} class="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-[color:rgba(var(--accent),.12)]">
    {#if open}
      <ChevronDown size={14} class="text-[color:rgb(var(--ink-faint))]" />
    {:else}
      <ChevronRight size={14} class="text-[color:rgb(var(--ink-faint))]" />
    {/if}
    <DataNodeIcon size={14} class="text-[color:rgb(var(--ink-faint))]" />
    <span class="text-[13px] font-medium flex-1 text-[color:var(--ink)]">{title}</span>
    <span class="text-[11px] text-[color:rgb(var(--ink-faint))]">{fields.length} fields</span>
  </button>
  {#if open}
    <table class="w-full text-[12px] border-t border-[color:var(--hair)]">
      <tbody>
        {#each fields as f (f.name)}
          <tr class="border-b border-[color:rgba(var(--accent),.10)] last:border-0">
            <td class="px-3 py-1.5 font-[family-name:var(--font-mono)] text-[color:var(--ink-dim)]">{f.name}</td>
            <td class="px-3 py-1.5 text-[color:rgb(var(--ink-faint))]">{f.type.name}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
