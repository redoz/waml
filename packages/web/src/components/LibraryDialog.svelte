<script lang="ts">
  // Mirrors packages/web/src/components/LibraryDialog.tsx.
  import { ChevronRight, ChevronDown, X, Rocket } from "lucide-svelte";
  import { build_model } from "@uaml/okf";
  import { toModelGraph, emptyOverlay, type RustModel } from "@uaml/core/state/overlay";
  import { TEMPLATES, INDUSTRY_TEMPLATES, DATASET_TEMPLATES, type Template } from "@uaml/core/templates";
  import { JoinIcon, LibraryIcon } from "../lib/icons";
  import MartRow from "./MartRow.svelte";

  type Bundle = [string, string][];

  let { onUse, onClose }: {
    onUse: (bundle: Bundle, name: string) => void;
    onClose: () => void;
  } = $props();

  let openId = $state<string | null>(TEMPLATES[0]?.id ?? null);

  function toggle(id: string) {
    openId = openId === id ? null : id;
  }

  // Derive the preview graph from the template's committed bundle (WASM core is
  // ready — the app awaited initWasm() at bootstrap).
  const deriveGraph = (bundle: Bundle) => toModelGraph(build_model(bundle) as unknown as RustModel, emptyOverlay());
</script>

{#snippet templateRow(t: Template)}
  {@const open = openId === t.id}
  {@const graph = deriveGraph(t.bundle)}
  {@const nodes = graph.nodes}
  {@const edges = graph.edges}
  <div class="shrink-0 rounded-xl border border-[#e2e6ec] overflow-hidden">
    <div
      onclick={() => toggle(t.id)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          toggle(t.id);
        }
      }}
      role="button"
      tabindex="0"
      aria-label={t.name}
      aria-expanded={open}
      class="flex items-center gap-3 px-4 py-3 hover:bg-[#f8fafc] text-left cursor-pointer"
    >
      {#if open}
        <ChevronDown size={16} class="text-slate-400 flex-shrink-0" />
      {:else}
        <ChevronRight size={16} class="text-slate-400 flex-shrink-0" />
      {/if}
      <div class="flex-1 min-w-0">
        <div class="text-[14px] font-semibold truncate">{t.name}</div>
        <div class="text-[12px] text-slate-500 truncate">{t.description}</div>
      </div>
      <span class="text-[11px] text-slate-500 whitespace-nowrap flex-shrink-0">{nodes.length} marts · {edges.length} links</span>
      <button
        onclick={(e) => { e.stopPropagation(); onUse(t.bundle.map(([p, m]) => [p, m]), t.name); }}
        title="Roll out this model onto the canvas"
        class="flex items-center gap-[6px] rounded-lg bg-[#1e88e5] px-3 py-[6px] text-[12px] font-semibold text-white hover:bg-[#1976d2] whitespace-nowrap"
      >
        <Rocket size={13} /> Use
      </button>
    </div>

    {#if open}
      <div class="px-4 pb-4 pt-1 bg-[#fbfcfe] border-t border-[#eef1f5] overflow-y-auto" style="max-height: 46vh">
        <div class="flex flex-col gap-1.5 mt-2">
          {#each nodes as n (n.key)}
            <MartRow title={n.title} fields={n.attributes} />
          {/each}
        </div>

        {#if edges.length > 0}
          <div class="mt-3">
            <div class="text-[10.5px] font-semibold uppercase tracking-wide text-slate-500 mb-1.5">Relationships</div>
            <ul class="flex flex-col gap-1">
              {#each edges as e (e.id)}
                {@const from = nodes.find(n => n.key === e.from)?.title ?? e.from}
                {@const to = nodes.find(n => n.key === e.to)?.title ?? e.to}
                <li class="flex items-center gap-2 text-[12px] text-slate-600">
                  <JoinIcon size={13} class="text-slate-400 flex-shrink-0" />
                  <span><b class="text-slate-800">{from}</b> {e.bidirectional ? "↔" : "→"} <b class="text-slate-800">{to}</b></span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onclick={onClose}>
  <div
    class="w-[620px] max-h-[88vh] flex flex-col overflow-hidden rounded-2xl border border-[#d8dee8] bg-white shadow-2xl"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-center gap-2 px-5 py-4 border-b border-[#d8dee8] flex-shrink-0">
      <LibraryIcon size={18} class="text-[#1e88e5]" />
      <h2 class="text-[15px] font-semibold flex-1">Template library</h2>
      <button onclick={onClose} class="text-slate-400 hover:text-slate-700"><X size={18} /></button>
    </div>

    <div class="flex-1 min-h-0 overflow-y-auto p-3 flex flex-col gap-2">
      <div class="px-1 text-[10.5px] font-semibold uppercase tracking-wide text-slate-500">Industry templates</div>
      {#each INDUSTRY_TEMPLATES as t (t.id)}
        {@render templateRow(t)}
      {/each}
      <div class="px-1 pt-2 text-[10.5px] font-semibold uppercase tracking-wide text-slate-500">Public datasets</div>
      {#each DATASET_TEMPLATES as t (t.id)}
        {@render templateRow(t)}
      {/each}
    </div>
  </div>
</div>
