<script lang="ts">
  // Confirmation before starting a new project. Not destructive — everything
  // autosaves, so this only guards the context switch — but follows the same
  // modal pattern as ClearCanvasDialog.svelte (overlay, card, header X, footer
  // Cancel + confirm) with a primary-blue confirm button instead of the red
  // destructive treatment.
  let { onConfirm, onClose }: {
    onConfirm: () => void;
    onClose: () => void;
  } = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div class="bg-white rounded-xl shadow-xl w-[460px] max-w-[95vw] p-6 flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-slate-900">Create a new project</h2>
      <button onclick={onClose} class="text-slate-400 hover:text-slate-700 text-xl leading-none px-1">✕</button>
    </div>

    <p class="text-[13px] text-slate-600">
      This will close the current project - your work is saved.
    </p>

    <div class="flex items-center justify-end gap-2">
      <button
        onclick={onClose}
        class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
      <button
        onclick={onConfirm}
        class="text-[13px] font-[550] bg-[#1e88e5] text-white border border-[#1e88e5] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#1976d2]"
      >
        Create new
      </button>
    </div>
  </div>
</div>
