<!-- packages/web/src/components/inspector/ObjectInspectorReadonly.svelte -->
<script lang="ts">
  import type { ModelNode, ModelEdge } from "@waml/okf";
  import { nodeAssociations } from "./associations";
  import { labelCls } from "./field-styles";

  let { node, nodes = [], edges = [], onSelectAssociation }: {
    node: ModelNode;
    nodes?: ModelNode[];
    edges?: ModelEdge[];
    /** Clicking an association row selects that edge. */
    onSelectAssociation?: (edgeId: string) => void;
  } = $props();

  const valueCls = "text-[13px] text-[color:var(--ink)] whitespace-pre-wrap break-words";
  const emptyCls = "text-[13px] text-[color:rgb(var(--ink-faint))] italic";

  const isEnum = $derived(node.type === "uml.Enum");
  const associations = $derived(nodeAssociations(node, edges, nodes));
</script>

<div class="flex flex-col gap-[15px]">
  <div>
    <span class={labelCls}>Title</span>
    {#if node.concept.title?.trim()}
      <div class={valueCls}>{node.concept.title}</div>
    {:else}
      <div class={emptyCls}>Untitled</div>
    {/if}
  </div>
  <div>
    <span class={labelCls}>Description</span>
    {#if node.concept.description?.trim()}
      <div class={valueCls}>{node.concept.description}</div>
    {:else}
      <div class={emptyCls}>No description</div>
    {/if}
  </div>
  <div class="flex gap-[10px] items-start">
    <div class="flex-1">
      <span class={labelCls}>Type</span>
      <div class={valueCls}>{node.type}</div>
    </div>
    {#if node.abstract}
      <span class="text-[12px] font-semibold text-[color:rgb(var(--accent))] bg-[color:rgba(var(--accent),.12)] rounded-[var(--round-chip)] px-2 py-1">abstract</span>
    {/if}
  </div>
  <div>
    <span class={labelCls}>Stereotypes</span>
    {#if node.stereotypes.length > 0}
      <div class={valueCls}>{node.stereotypes.map((s) => `«${s}»`).join(" ")}</div>
    {:else}
      <div class={emptyCls}>None</div>
    {/if}
  </div>
  <div>
    <span class={labelCls}>Associations</span>
    {#if associations.length > 0}
      <ul class="flex flex-col gap-[4px]">
        {#each associations as a (a.id)}
          <li>
            <button
              type="button"
              onclick={() => onSelectAssociation?.(a.id)}
              class="w-full text-left text-[13px] text-[color:var(--ink)] break-words flex items-baseline gap-[6px] rounded-[var(--round-chip)] -mx-1 px-1 py-[2px] hover:bg-[color:rgba(var(--accent),.10)] focus:outline-none focus:ring-2 focus:ring-[color:rgba(var(--accent),.20)]"
            >
              <span class="text-[color:rgb(var(--ink-faint))] font-mono">{a.outgoing ? "→" : "←"}</span>
              <span class="font-semibold">{a.otherTitle}</span>
              <span class="text-[11px] text-[color:rgb(var(--ink-faint))]">{a.kind}{a.role ? ` (${a.role})` : ""}{a.multiplicity ? ` [${a.multiplicity}]` : ""}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <div class={emptyCls}>No associations</div>
    {/if}
  </div>
  {#if isEnum}
    <div>
      <span class={labelCls}>Values</span>
      {#if (node.values ?? []).length > 0}
        <ul class="text-[13px] text-[color:var(--ink)] list-disc pl-5">
          {#each node.values ?? [] as v (v)}
            <li>{v}</li>
          {/each}
        </ul>
      {:else}
        <div class={emptyCls}>No values</div>
      {/if}
    </div>
  {:else}
    <div>
      <span class={labelCls}>Attributes</span>
      {#if node.attributes.length > 0}
        <ul class="flex flex-col gap-[4px]">
          {#each node.attributes as a, i (i)}
            <li class="text-[13px] text-[color:var(--ink)] font-mono break-words">
              {a.visibility ?? ""}{a.name}: {a.type.name}{a.multiplicity && a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}
            </li>
          {/each}
        </ul>
      {:else}
        <div class={emptyCls}>No attributes</div>
      {/if}
    </div>
  {/if}
</div>
