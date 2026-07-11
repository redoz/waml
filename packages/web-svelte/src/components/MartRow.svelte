<script lang="ts">
  // Private helper for LibraryDialog.svelte — mirrors the MartRow() function
  // in packages/web/src/components/LibraryDialog.tsx. Split into its own
  // component because it owns its own `open` toggle state, which a snippet
  // can't hold cleanly.
  import { ChevronRight, ChevronDown } from "lucide-svelte";
  import { DataMartIcon } from "../lib/icons";

  let { title, fields }: {
    title: string;
    fields: { name: string; type: { name: string } }[];
  } = $props();

  let open = $state(false);
</script>

<div class="rounded-lg border border-[#e9edf2] bg-white">
  <button onclick={() => (open = !open)} class="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-[#f8fafc]">
    {#if open}
      <ChevronDown size={14} class="text-slate-400" />
    {:else}
      <ChevronRight size={14} class="text-slate-400" />
    {/if}
    <DataMartIcon size={14} class="text-slate-500" />
    <span class="text-[13px] font-medium flex-1">{title}</span>
    <span class="text-[11px] text-slate-500">{fields.length} fields</span>
  </button>
  {#if open}
    <table class="w-full text-[12px] border-t border-[#eef1f5]">
      <tbody>
        {#each fields as f (f.name)}
          <tr class="border-b border-[#f3f5f8] last:border-0">
            <td class="px-3 py-1.5 font-mono text-slate-700">{f.name}</td>
            <td class="px-3 py-1.5 text-slate-500">{f.type.name}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
