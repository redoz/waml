<script lang="ts">
  // Confirmation before starting a new project. Not destructive — everything
  // autosaves, so this only guards the context switch — but follows the same
  // modal pattern as ClearCanvasDialog.svelte (overlay, card, header X, footer
  // Cancel + confirm) with a primary accent confirm button instead of the red
  // destructive treatment.
  import { X } from "lucide-svelte";

  let { onConfirm, onClose }: {
    onConfirm: () => void;
    onClose: () => void;
  } = $props();
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
      <h2 class="text-[15px] font-semibold text-[color:var(--ink)]">Create a new project</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <p class="text-[13px] text-[color:var(--ink-dim)]">
      This will close the current project - your work is saved.
    </p>

    <div class="flex items-center justify-end gap-2">
      <button
        onclick={onClose}
        class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:bg-[color:rgba(var(--accent),.10)]"
      >
        Cancel
      </button>
      <button
        onclick={onConfirm}
        class="text-[13px] font-[600] bg-[color:rgb(var(--accent))] text-white border border-[color:rgb(var(--accent))] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:brightness-95"
      >
        Create new
      </button>
    </div>
    </div>
  </div>
</div>
