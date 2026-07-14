<script lang="ts">
  // Mirrors packages/web/src/components/TemplateApplyDialog.tsx.
  // Shown when "Use" is clicked on a template while the canvas already has
  // content. Mirrors the OKF import dialogs: choose Replace vs Merge
  // and see how many nodes and relationships will be added before committing.
  import { build_model } from "@waml/wasm";

  type Bundle = [string, string][];

  let { bundle, name, onConfirm, onClose }: {
    bundle: Bundle;
    name: string;
    onConfirm: (mode: "replace" | "merge") => void;
    onClose: () => void;
  } = $props();

  let mode = $state<"replace" | "merge">("replace");

  const modes = ["replace", "merge"] as const;

  // Preview counts from the WASM core (the app awaited initWasm() at bootstrap).
  const counts = $derived(build_model(bundle) as { nodes: unknown[]; edges: unknown[] });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[60] flex items-center justify-center bg-black/40"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div class="bg-white rounded-xl shadow-xl w-[440px] max-w-[95vw] p-6 flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-slate-900">Add “{name}” to the canvas</h2>
      <button
        onclick={onClose}
        class="text-slate-400 hover:text-slate-700 text-xl leading-none px-1"
      >
        ✕
      </button>
    </div>

    <p class="text-[13px] text-slate-600 -mt-1">
      Your canvas already has content. Choose how to apply this template.
    </p>

    <div class="flex flex-col gap-1.5 border-t border-slate-100 pt-3">
      <span class="text-[12px] font-medium text-slate-500">When applying to the canvas</span>
      {#each modes as m (m)}
        <label class="flex items-center gap-2 text-[13px] text-slate-800 cursor-pointer">
          <input type="radio" name="template-mode" value={m} bind:group={mode} />
          {m === "replace" ? "Replace the canvas" : "Merge into the canvas"}
        </label>
      {/each}
      <p class="text-[12px] text-slate-500">
        Will import {counts.nodes.length} nodes, {counts.edges.length} relationships.
      </p>
    </div>

    <div class="flex gap-2 justify-end">
      <button
        onclick={onClose}
        class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
      <button
        onclick={() => onConfirm(mode)}
        class="text-[13px] font-[550] bg-[#1e88e5] text-white border border-[#1e88e5] rounded-lg px-4 py-[7px] cursor-pointer hover:bg-[#1976d2]"
      >
        Apply
      </button>
    </div>
  </div>
</div>
