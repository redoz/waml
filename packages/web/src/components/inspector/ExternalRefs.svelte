<script lang="ts">
  import type { ModelNode, ModelEdge, Diagram } from "@waml/okf";
  import { labelCls } from "./field-styles";

  let { nodeKey, nodes, edges, members, diagrams, onNavigate }: {
    nodeKey: string;
    nodes: ModelNode[];
    edges: ModelEdge[];
    members: string[];
    diagrams: Diagram[];
    onNavigate: (diagramKey: string, nodeKey: string) => void;
  } = $props();

  // The spec's "isolate a domain, still see other sources" behavior: relationships
  // whose other end is off-diagram surface here as navigable chips.
  const refs = $derived.by(() => {
    const memberSet = new Set(members);
    const byKey = new Map(nodes.map(n => [n.key, n]));
    const result: { key: string; label: string; other: string }[] = [];
    for (const e of edges) {
      if (e.from === nodeKey && !memberSet.has(e.to) && byKey.has(e.to)) {
        result.push({ key: e.id, label: `${e.kind} → ${byKey.get(e.to)!.concept.title ?? e.to}`, other: e.to });
      } else if (e.to === nodeKey && !memberSet.has(e.from) && byKey.has(e.from)) {
        result.push({ key: e.id, label: `${byKey.get(e.from)!.concept.title ?? e.from} → ${e.kind}`, other: e.from });
      }
    }
    return result;
  });

  const diagramFor = (k: string) => diagrams.find(d => d.members.includes(k))?.key;
</script>

{#if refs.length > 0}
  <div>
    <span class={labelCls}>
      External references
    </span>
    <div class="flex flex-wrap gap-[6px]">
      {#each refs as r (r.key)}
        {@const target = diagramFor(r.other)}
        <button
          disabled={!target}
          onclick={() => target && onNavigate(target, r.other)}
          title={target ? "Open the diagram containing this node" : "Not on any diagram"}
          class="rounded-[var(--round-chip)] border border-[color:rgba(var(--accent),.30)] bg-white px-[10px] py-[4px] text-[11.5px] text-[color:rgb(var(--ink-faint))] hover:border-[color:rgb(var(--accent))] hover:text-[color:rgb(var(--accent))] disabled:opacity-50"
        >
          {r.label}
        </button>
      {/each}
    </div>
  </div>
{/if}
