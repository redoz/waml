<!-- packages/web/src/components/inspector/ObjectInspectorReadonly.svelte -->
<script lang="ts">
  import type { ModelNode } from "@waml/okf";

  let { node }: { node: ModelNode } = $props();

  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
  const valueCls = "text-[13px] text-slate-900 whitespace-pre-wrap break-words";
  const emptyCls = "text-[13px] text-slate-400 italic";

  const isEnum = $derived(node.type === "uml.Enum");
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
      <span class="text-[12px] font-semibold text-[#1e88e5] bg-[#e6f1fb] rounded px-2 py-1">abstract</span>
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
  {#if isEnum}
    <div>
      <span class={labelCls}>Values</span>
      {#if (node.values ?? []).length > 0}
        <ul class="text-[13px] text-slate-900 list-disc pl-5">
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
            <li class="text-[13px] text-slate-900 font-mono break-words">
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
