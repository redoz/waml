<script lang="ts">
  // Mirrors packages/web/src/components/WelcomeDialog.tsx.
  import { X, Rocket, Plus, Download, ExternalLink } from "lucide-svelte";
  import { build_model } from "@waml/wasm";
  import { toModelGraph, emptyOverlay, type RustModel } from "@waml/core/state/overlay";
  import { TEMPLATES, type Template } from "@waml/core/templates";
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
  <div class="flex items-center gap-3 rounded-[var(--round-chip)] border border-[color:var(--hair)] px-4 py-3 hover:bg-[color:rgba(var(--accent),.10)]">
    <div class="flex-1 min-w-0">
      <div class="text-[14px] font-semibold text-[color:var(--ink)]">{t.name}</div>
      <div class="text-[12px] text-[color:rgb(var(--ink-faint))] truncate">{t.description}</div>
    </div>
    <span class="text-[11px] text-[color:rgb(var(--ink-faint))] whitespace-nowrap">{graph.nodes.length} nodes · {graph.edges.length} links</span>
    <button
      onclick={() => onUseTemplate(t.bundle.map(([p, m]) => [p, m]), t.name)}
      title={`Roll out the ${t.name} model`}
      class="flex items-center gap-[6px] rounded-[var(--round-chip)] bg-[color:rgb(var(--accent))] px-3 py-[6px] text-[12px] font-semibold text-white hover:brightness-95 whitespace-nowrap"
    >
      <Rocket size={13} /> Use
    </button>
  </div>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30 p-3 sm:p-4"
  style="font-family: var(--font-ui);"
  onclick={onStartBlank}
>
  <div
    class="hud-surface relative w-full max-w-[640px] max-h-[90vh] flex flex-col overflow-hidden"
    onclick={e => e.stopPropagation()}
  >
    <!-- Header -->
    <div class="relative z-[1] flex items-start gap-3 px-6 pt-5 pb-4 border-b border-[color:rgba(var(--accent),.22)] flex-shrink-0">
      <div class="flex-1">
        <h2 class="text-[17px] font-semibold tracking-[-0.2px] text-[color:var(--ink)]">Start your data model</h2>
        <p class="mt-1 text-[13px] leading-relaxed text-[color:rgb(var(--ink-faint))]">
          Pick a template to explore, start from a blank canvas, or import an existing model. It's free — no
          sign-in needed.
        </p>
      </div>
      <button
        onclick={onStartBlank}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <!-- Templates -->
    <div class="relative z-[1] overflow-y-auto px-4 py-3 flex flex-col gap-2">
      <div class="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wide text-[color:rgb(var(--ink-faint))]">
        <LibraryIcon size={14} class="text-[color:rgb(var(--accent))]" /> Start from a template
      </div>
      {#each TEMPLATES as t (t.id)}
        {@render templateChoice(t)}
      {/each}
    </div>

    <!-- Footer: start blank / import -->
    <div class="relative z-[1] flex items-center flex-wrap gap-x-3 gap-y-2 px-6 py-4 border-t border-[color:rgba(var(--accent),.22)] flex-shrink-0">
      <button
        onclick={onStartBlank}
        class="flex items-center gap-[7px] text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-3 py-[8px] cursor-pointer hover:bg-[color:rgba(var(--accent),.10)]"
      >
        <Plus size={15} /> Start blank
      </button>
      <button
        onclick={onImport}
        class="flex items-center gap-[7px] text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-3 py-[8px] cursor-pointer hover:bg-[color:rgba(var(--accent),.10)]"
      >
        <Download size={15} /> Import OKF
      </button>
      <div class="flex-1"></div>
      <a
        href="/okf-format.md"
        target="_blank"
        rel="noopener"
        class="flex items-center gap-[5px] text-[12.5px] font-[600] text-[color:rgb(var(--accent))] hover:underline"
      >
        Import guide <ExternalLink size={13} />
      </a>
    </div>
  </div>
</div>
