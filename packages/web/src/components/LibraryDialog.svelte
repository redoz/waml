<script lang="ts">
  // Mirrors packages/web/src/components/LibraryDialog.tsx.
  import { ChevronRight, ChevronDown, X, Rocket } from "lucide-svelte";
  import { build_model } from "@waml/wasm";
  import { toModelGraph, emptyOverlay, type RustModel } from "@waml/core/state/overlay";
  import { TEMPLATES, type Template } from "@waml/core/templates";
  import { JoinIcon, LibraryIcon } from "../lib/icons";
  import NodeRow from "./NodeRow.svelte";

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
  <div class="shrink-0 rounded-[var(--round-chip)] border border-[color:var(--hair)] overflow-hidden">
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
      class="flex items-center gap-3 px-4 py-3 hover:bg-[color:rgba(var(--accent),.10)] text-left cursor-pointer"
    >
      {#if open}
        <ChevronDown size={16} class="text-[color:rgb(var(--ink-faint))] flex-shrink-0" />
      {:else}
        <ChevronRight size={16} class="text-[color:rgb(var(--ink-faint))] flex-shrink-0" />
      {/if}
      <div class="flex-1 min-w-0">
        <div class="text-[14px] font-semibold truncate text-[color:var(--ink)]">{t.name}</div>
        <div class="text-[12px] text-[color:rgb(var(--ink-faint))] truncate">{t.description}</div>
      </div>
      <span class="text-[11px] text-[color:rgb(var(--ink-faint))] whitespace-nowrap flex-shrink-0">{nodes.length} nodes · {edges.length} links</span>
      <button
        onclick={(e) => { e.stopPropagation(); onUse(t.bundle.map(([p, m]) => [p, m]), t.name); }}
        title="Roll out this model onto the canvas"
        class="flex items-center gap-[6px] rounded-[var(--round-chip)] bg-[color:rgb(var(--accent))] px-3 py-[6px] text-[12px] font-semibold text-white hover:brightness-95 whitespace-nowrap"
      >
        <Rocket size={13} /> Use
      </button>
    </div>

    {#if open}
      <div class="px-4 pb-4 pt-1 bg-[color:rgba(var(--accent),.04)] border-t border-[color:rgba(var(--accent),.14)] overflow-y-auto" style="max-height: 46vh">
        <div class="flex flex-col gap-1.5 mt-2">
          {#each nodes as n (n.key)}
            <NodeRow title={n.concept.title ?? "Untitled"} fields={n.attributes} />
          {/each}
        </div>

        {#if edges.length > 0}
          <div class="mt-3">
            <div class="text-[10.5px] font-semibold uppercase tracking-wide text-[color:rgb(var(--ink-faint))] mb-1.5">Relationships</div>
            <ul class="flex flex-col gap-1">
              {#each edges as e (e.id)}
                {@const from = nodes.find(n => n.key === e.from)?.concept.title ?? e.from}
                {@const to = nodes.find(n => n.key === e.to)?.concept.title ?? e.to}
                <li class="flex items-center gap-2 text-[12px] text-[color:var(--ink-dim)]">
                  <JoinIcon size={13} class="text-[color:rgb(var(--ink-faint))] flex-shrink-0" />
                  <span><b class="text-[color:var(--ink)]">{from}</b> {e.bidirectional ? "↔" : "→"} <b class="text-[color:var(--ink)]">{to}</b></span>
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
<div class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30" style="font-family: var(--font-ui);" onclick={onClose}>
  <div
    class="hud-surface relative w-[620px] max-h-[88vh] flex flex-col overflow-hidden"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="relative z-[1] flex items-center gap-2 px-5 py-4 border-b border-[color:rgba(var(--accent),.22)] flex-shrink-0">
      <LibraryIcon size={18} class="text-[color:rgb(var(--accent))]" />
      <h2 class="text-[15px] font-semibold flex-1 text-[color:var(--ink)]">Template library</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <div class="relative z-[1] flex-1 min-h-0 overflow-y-auto p-3 flex flex-col gap-2">
      <div class="px-1 text-[10.5px] font-semibold uppercase tracking-wide text-[color:rgb(var(--ink-faint))]">Built-in templates</div>
      {#each TEMPLATES as t (t.id)}
        {@render templateRow(t)}
      {/each}
    </div>
  </div>
</div>
