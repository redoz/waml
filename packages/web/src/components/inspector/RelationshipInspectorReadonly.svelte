<!-- packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte -->
<script lang="ts">
  import type { ModelEdge, ModelNode } from "@waml/okf";
  import { ENDED_KINDS } from "@waml/okf";

  let { edge, fromNode, toNode }: {
    edge: ModelEdge;
    fromNode?: ModelNode;
    toNode?: ModelNode;
  } = $props();

  const fromTitle = $derived(fromNode?.concept.title?.trim() || "Source");
  const toTitle = $derived(toNode?.concept.title?.trim() || "Target");
  const hasEnds = $derived(ENDED_KINDS.has(edge.kind));

  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
  const valueCls = "text-[13px] text-slate-900";
</script>

<div class="flex flex-col gap-[15px]">
  <div class="text-[13px] text-slate-500">
    <strong class="text-slate-900">{fromTitle}</strong> → <strong class="text-slate-900">{toTitle}</strong>
  </div>
  <div>
    <span class={labelCls}>Kind</span>
    <div class={valueCls}>{edge.kind}</div>
  </div>
  {#if hasEnds}
    <div class="flex flex-col gap-[10px]">
      <div class="flex gap-[6px]">
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{fromTitle} multiplicity</span>
          <div class={valueCls}>{edge.fromEnd.multiplicity ?? "—"}</div>
        </div>
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{fromTitle} role</span>
          <div class={valueCls}>{edge.fromEnd.role ?? "—"}</div>
        </div>
      </div>
      <div class="flex gap-[6px]">
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{toTitle} multiplicity</span>
          <div class={valueCls}>{edge.toEnd.multiplicity ?? "—"}</div>
        </div>
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{toTitle} role</span>
          <div class={valueCls}>{edge.toEnd.role ?? "—"}</div>
        </div>
      </div>
    </div>
  {/if}
  {#if edge.kind === "associates" && edge.bidirectional}
    <div class={valueCls}>Bidirectional</div>
  {/if}
</div>
