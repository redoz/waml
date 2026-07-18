<script lang="ts">
  // Mirrors packages/web/src/components/ClearCanvasDialog.tsx.
  //
  // Destructive-action confirmation before clearing the whole canvas. Clearing
  // is permanent and can't be undone, so we nudge the user to export an OKF
  // bundle to their computer first. Two destructive paths (export-then-delete,
  // or just delete) plus Cancel.
  import { X } from "lucide-svelte";

  let { counts, onDelete, onExportAndDelete, onClose }: {
    counts: { nodes: number; relationships: number };
    onDelete: () => void;
    onExportAndDelete: () => void;
    onClose: () => void;
  } = $props();

  const empty = $derived(counts.nodes === 0 && counts.relationships === 0);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30"
  style="font-family: var(--font-ui);"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div
    class="hud-surface relative w-[460px] max-w-[95vw] flex flex-col overflow-hidden"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="relative z-[1] flex flex-col gap-4 p-6">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-[color:var(--ink)]">Clear canvas</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <div class="rounded-[var(--round-chip)] border border-[color:rgba(var(--danger),.30)] bg-[color:rgba(var(--danger),.10)] px-4 py-3 text-[13px] leading-relaxed text-[color:rgb(var(--danger))]">
      <!-- Whitespace butts directly against the {#if}/{/if} tags so Svelte's
           whitespace-collapse doesn't inject a stray space before the period
           (empty case) or after the counts (matches ClearCanvasDialog.tsx). -->
      This permanently deletes everything on the canvas{#if !empty}{" "}— <span class="font-semibold">{counts.nodes} {counts.nodes === 1 ? "node" : "nodes"}</span> and <span class="font-semibold">{counts.relationships} {counts.relationships === 1 ? "relationship" : "relationships"}</span>{/if}. This can't be undone.
    </div>

    <p class="text-[13px] text-[color:var(--ink-dim)]">
      We recommend exporting an <span class="font-semibold">OKF</span> bundle to your computer first so you can re-import this model later.
    </p>

    <div class="flex items-center justify-between gap-2">
      <button
        onclick={onClose}
        class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:bg-[color:rgba(var(--accent),.10)]"
      >
        Cancel
      </button>
      <div class="flex gap-2">
        <button
          onclick={onExportAndDelete}
          class="text-[13px] font-[600] border border-[color:rgb(var(--danger))] bg-white text-[color:rgb(var(--danger))] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:bg-[color:rgba(var(--danger),.10)]"
        >
          Export OKF & delete
        </button>
        <button
          onclick={onDelete}
          class="text-[13px] font-[600] bg-[color:rgb(var(--danger))] text-white border border-[color:rgb(var(--danger))] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:brightness-95"
        >
          Delete
        </button>
      </div>
    </div>
    </div>
  </div>
</div>
