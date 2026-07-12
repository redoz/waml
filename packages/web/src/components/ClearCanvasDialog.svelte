<script lang="ts">
  // Mirrors packages/web/src/components/ClearCanvasDialog.tsx.
  //
  // Destructive-action confirmation before clearing the whole canvas. Clearing
  // is permanent and can't be undone, so we nudge the user to export an OKF
  // bundle to their computer first. Two destructive paths (export-then-delete,
  // or just delete) plus Cancel.
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
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div class="bg-white rounded-xl shadow-xl w-[460px] max-w-[95vw] p-6 flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-slate-900">Clear canvas</h2>
      <button onclick={onClose} class="text-slate-400 hover:text-slate-700 text-xl leading-none px-1">✕</button>
    </div>

    <div class="rounded-lg border border-[#f4caca] bg-[#fdf2f2] px-4 py-3 text-[13px] leading-relaxed text-[#7f1d1d]">
      <!-- Whitespace butts directly against the {#if}/{/if} tags so Svelte's
           whitespace-collapse doesn't inject a stray space before the period
           (empty case) or after the counts (matches ClearCanvasDialog.tsx). -->
      This permanently deletes everything on the canvas{#if !empty}{" "}— <span class="font-semibold">{counts.nodes} {counts.nodes === 1 ? "node" : "nodes"}</span> and <span class="font-semibold">{counts.relationships} {counts.relationships === 1 ? "relationship" : "relationships"}</span>{/if}. This can't be undone.
    </div>

    <p class="text-[13px] text-slate-600">
      We recommend exporting an <span class="font-semibold">OKF</span> bundle to your computer first so you can re-import this model later.
    </p>

    <div class="flex items-center justify-between gap-2">
      <button
        onclick={onClose}
        class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
      <div class="flex gap-2">
        <button
          onclick={onExportAndDelete}
          class="text-[13px] font-[550] border border-[#dc2626] bg-white text-[#dc2626] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#fdf2f2]"
        >
          Export OKF & delete
        </button>
        <button
          onclick={onDelete}
          class="text-[13px] font-[550] bg-[#dc2626] text-white border border-[#dc2626] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#b91c1c]"
        >
          Delete
        </button>
      </div>
    </div>
  </div>
</div>
