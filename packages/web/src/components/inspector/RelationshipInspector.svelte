<script lang="ts">
  import type { ModelEdge, ModelNode, RelationshipKind, RelEnd } from "@waml/okf";
  import { RELATIONSHIP_KINDS, ENDED_KINDS } from "@waml/okf";
  import InfoTip from "./InfoTip.svelte";

  let { edge, fromNode, toNode, onUpdate }: {
    edge: ModelEdge;
    fromNode?: ModelNode;
    toNode?: ModelNode;
    onUpdate: (patch: Partial<ModelEdge>) => void;
  } = $props();

  const KIND_HELP: Record<RelationshipKind, string> = {
    associates: "Plain association — solid line, arrowhead on navigable end(s).",
    aggregates: "Shared aggregation — hollow diamond on this (whole) end.",
    composes: "Composition — filled diamond on this (whole) end; parts live and die with the whole.",
    specializes: "Generalization — hollow triangle at the parent (near→far reads child→parent).",
    implements: "Realization — dashed line, hollow triangle at the interface.",
    depends: "Dependency — dashed open arrow at the target.",
    annotates: "Note anchor — uml.Note only; never selectable here.",
  };

  // `annotates` is a uml.Note-only verb (anchors live on the note node, not on edges) — hide it from the edge verb select.
  const EDGE_KINDS = RELATIONSHIP_KINDS.filter(k => k !== "annotates");

  const inputCls = "w-full text-[13px] px-[10px] py-[8px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

  const fromTitle = $derived(fromNode?.concept.title ?? "Source");
  const toTitle = $derived(toNode?.concept.title ?? "Target");
  const hasEnds = $derived(ENDED_KINDS.has(edge.kind));
</script>

{#snippet endEditor(title: string, end: RelEnd, onChange: (end: RelEnd) => void)}
  <div class="flex gap-[6px]">
    <label class="flex-1 text-[11px] text-slate-500">
      {title} multiplicity
      <input
        aria-label={`${title} multiplicity`}
        type="text"
        value={end.multiplicity ?? ""}
        placeholder="1, 0..1, *"
        oninput={(e) => onChange({ ...end, multiplicity: e.currentTarget.value || undefined })}
        class={inputCls}
      />
    </label>
    <label class="flex-1 text-[11px] text-slate-500">
      {title} role
      <input
        aria-label={`${title} role`}
        type="text"
        value={end.role ?? ""}
        placeholder="role"
        oninput={(e) => onChange({ ...end, role: e.currentTarget.value || undefined })}
        class={inputCls}
      />
    </label>
  </div>
{/snippet}

<div class="flex flex-col gap-[15px]">
  <div class="text-[13px] text-slate-500">
    <strong class="text-slate-900">{fromTitle}</strong> → <strong class="text-slate-900">{toTitle}</strong>
  </div>
  <div>
    <label for="rel-kind" class="flex items-center gap-[5px] text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
      Kind <InfoTip text={KIND_HELP[edge.kind]} />
    </label>
    <select
      id="rel-kind"
      aria-label="Kind"
      value={edge.kind}
      onchange={(e) => onUpdate({ kind: e.currentTarget.value as RelationshipKind })}
      class={inputCls}
    >
      {#each EDGE_KINDS as k (k)}
        <option value={k}>{k}</option>
      {/each}
    </select>
  </div>
  {#if hasEnds}
    <div class="flex flex-col gap-[10px]">
      {@render endEditor(fromTitle, edge.fromEnd, (fromEnd) => onUpdate({ fromEnd }))}
      {@render endEditor(toTitle, edge.toEnd, (toEnd) => onUpdate({ toEnd }))}
    </div>
  {/if}
  {#if edge.kind === "associates"}
    <label class="flex items-start gap-[9px] cursor-pointer">
      <input
        type="checkbox"
        checked={edge.bidirectional}
        onchange={(e) => onUpdate({
          bidirectional: e.currentTarget.checked,
          fromEnd: { ...edge.fromEnd, navigable: e.currentTarget.checked ? true : undefined },
          toEnd: { ...edge.toEnd, navigable: true },
        })}
        class="w-4 h-4 mt-[1px] accent-[#1e88e5] cursor-pointer"
      />
      <span class="text-[12.5px]">
        <strong class="text-[13px]">Bidirectional</strong>
        <span class="text-slate-500 mt-[2px] leading-[1.4] block">Both ends navigable — arrowheads on both ends.</span>
      </span>
    </label>
  {/if}
</div>
