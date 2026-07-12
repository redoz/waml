<script lang="ts">
  // Mirrors packages/web/src/components/WelcomeDialog.tsx.
  import { X, Rocket, Plus, Download, ExternalLink } from "lucide-svelte";
  import { build_model } from "@uaml/okf";
  import { toModelGraph, emptyOverlay, type RustModel } from "@uaml/core/state/overlay";
  import { INDUSTRY_TEMPLATES, DATASET_TEMPLATES, type Template } from "@uaml/core/templates";
  import { LibraryIcon } from "../lib/icons";

  type Bundle = [string, string][];
  const deriveGraph = (bundle: Bundle) => toModelGraph(build_model(bundle) as unknown as RustModel, emptyOverlay());

  // First-screen chooser shown to brand-new visitors: pick a template (value
  // first), start blank, or import an existing model. Dismissing (X / backdrop)
  // is treated as "start blank".
  let {
    onUseTemplate,
    onStartBlank,
    onImport,
  }: {
    /** Roll a template onto the canvas. */
    onUseTemplate: (bundle: Bundle, name: string) => void;
    /** Dismiss and start from an empty canvas. */
    onStartBlank: () => void;
    /** Open the OKF import flow. */
    onImport: () => void;
  } = $props();
</script>

{#snippet templateChoice(t: Template)}
  {@const graph = deriveGraph(t.bundle)}
  <div class="flex items-center gap-3 rounded-xl border border-[#e2e6ec] px-4 py-3 hover:bg-[#f8fafc]">
    <div class="flex-1 min-w-0">
      <div class="text-[14px] font-semibold">{t.name}</div>
      <div class="text-[12px] text-slate-500 truncate">{t.description}</div>
    </div>
    <span class="text-[11px] text-slate-500 whitespace-nowrap">{graph.nodes.length} nodes · {graph.edges.length} links</span>
    <button
      onclick={() => onUseTemplate(t.bundle.map(([p, m]) => [p, m]), t.name)}
      title={`Roll out the ${t.name} model`}
      class="flex items-center gap-[6px] rounded-lg bg-[#1e88e5] px-3 py-[6px] text-[12px] font-semibold text-white hover:bg-[#1976d2] whitespace-nowrap"
    >
      <Rocket size={13} /> Use
    </button>
  </div>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-3 sm:p-4" onclick={onStartBlank}>
  <div
    class="w-full max-w-[640px] max-h-[90vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-2xl overflow-hidden"
    onclick={e => e.stopPropagation()}
  >
    <!-- Header -->
    <div class="flex items-start gap-3 px-6 pt-5 pb-4 border-b border-[#e6e9f0] flex-shrink-0">
      <div class="flex-1">
        <h2 class="text-[17px] font-semibold tracking-[-0.2px]">Start your data model</h2>
        <p class="mt-1 text-[13px] leading-relaxed text-slate-500">
          Pick a template to explore, start from a blank canvas, or import an existing model. It's free — no
          sign-in needed.
        </p>
      </div>
      <button onclick={onStartBlank} aria-label="Close" class="text-slate-400 hover:text-slate-700"><X size={18} /></button>
    </div>

    <!-- Templates -->
    <div class="overflow-y-auto px-4 py-3 flex flex-col gap-2">
      <div class="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
        <LibraryIcon size={14} class="text-[#1e88e5]" /> Start from a template
      </div>
      {#each INDUSTRY_TEMPLATES as t (t.id)}
        {@render templateChoice(t)}
      {/each}
      <div class="px-1 pt-2 text-[11px] font-semibold uppercase tracking-wide text-slate-500">Public datasets</div>
      {#each DATASET_TEMPLATES as t (t.id)}
        {@render templateChoice(t)}
      {/each}
    </div>

    <!-- Footer: start blank / import -->
    <div class="flex items-center flex-wrap gap-x-3 gap-y-2 px-6 py-4 border-t border-[#e6e9f0] flex-shrink-0">
      <button
        onclick={onStartBlank}
        class="flex items-center gap-[7px] text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-3 py-[8px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        <Plus size={15} /> Start blank
      </button>
      <button
        onclick={onImport}
        class="flex items-center gap-[7px] text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-3 py-[8px] cursor-pointer hover:bg-[#f1f3f7]"
      >
        <Download size={15} /> Import OKF
      </button>
      <div class="flex-1"></div>
      <a
        href="/okf-format.md"
        target="_blank"
        rel="noopener"
        class="flex items-center gap-[5px] text-[12.5px] font-[550] text-[#1e88e5] hover:underline"
      >
        Import guide <ExternalLink size={13} />
      </a>
    </div>
  </div>
</div>
