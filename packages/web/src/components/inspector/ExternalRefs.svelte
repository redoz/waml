<script lang="ts">
  import type { ModelNode, ModelEdge, Diagram } from "@waml/okf";

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
    <span class="block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
      External references
    </span>
    <div class="flex flex-wrap gap-[6px]">
      {#each refs as r (r.key)}
        {@const target = diagramFor(r.other)}
        <button
          disabled={!target}
          onclick={() => target && onNavigate(target, r.other)}
          title={target ? "Open the diagram containing this node" : "Not on any diagram"}
          class="rounded-full border border-[#d8dee8] bg-white px-[10px] py-[4px] text-[11.5px] text-slate-600 hover:border-[#1e88e5] hover:text-[#1e88e5] disabled:opacity-50"
        >
          {r.label}
        </button>
      {/each}
    </div>
  </div>
{/if}
