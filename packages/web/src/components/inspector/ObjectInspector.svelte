<script lang="ts">
  import type { ModelNode, ModelEdge, Attribute } from "@waml/okf";
  import AttributeEditor from "./AttributeEditor.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { nodeAssociations } from "./associations";
  import { getProfile } from "@waml/core/profiles";
  import { inputCls, labelCls } from "./field-styles";

  let { node, onUpdate, profileName, nodes = [], edges = [], onSelectAssociation }: {
    node: ModelNode;
    onUpdate: (patch: Partial<ModelNode>) => void;
    profileName?: string;
    nodes?: ModelNode[];
    edges?: ModelEdge[];
    /** Clicking an association row repoints the inspector at that edge. */
    onSelectAssociation?: (edgeId: string) => void;
  } = $props();

  let palette = $derived(getProfile(profileName).palette);
  let isEnum = $derived(node.type === "uml.Enum");
  let associations = $derived(nodeAssociations(node, edges, nodes));
</script>

<div class="flex flex-col gap-[15px]">
  <div>
    <label class={labelCls} for="oi-title">Title</label>
    <input id="oi-title" type="text" value={node.concept.title ?? ""} oninput={(e) => onUpdate({ concept: { ...node.concept, title: e.currentTarget.value } })} class={inputCls} />
  </div>
  <div>
    <label class={labelCls} for="oi-description">Description</label>
    <textarea
      id="oi-description"
      value={node.concept.description ?? ""}
      rows={3}
      oninput={(e) => onUpdate({ concept: { ...node.concept, description: e.currentTarget.value } })}
      class={`${inputCls} resize-y min-h-[60px]`}
    ></textarea>
  </div>
  <div class="flex gap-[10px]">
    <div class="flex-1">
      <label class={`${labelCls} flex items-center gap-[5px]`}>
        Type <InfoTip text="family.Metaclass dispatch key (e.g. uml.Class). Unknown values render as a generic box — never an error." />
      </label>
      <input
        type="text"
        list="okf-metaclasses"
        value={node.type}
        oninput={(e) => onUpdate({ type: e.currentTarget.value })}
        class={inputCls}
      />
      <datalist id="okf-metaclasses">
        {#each palette.metaclasses as t (t)}
          <option value={t}></option>
        {/each}
      </datalist>
    </div>
    <label class="flex items-end gap-[7px] pb-[9px] cursor-pointer text-[12.5px] text-slate-700">
      <input
        type="checkbox"
        checked={node.abstract ?? false}
        onchange={(e) => onUpdate({ abstract: e.currentTarget.checked || undefined })}
        class="w-4 h-4 accent-[#1e88e5] cursor-pointer"
      />
      abstract
    </label>
  </div>
  <div>
    <label class={`${labelCls} flex items-center gap-[5px]`}>
      Stereotypes <InfoTip text="Comma-separated, open set: entity, valueObject, aggregateRoot, service, domainEvent — invent any. Rendered as «guillemets»." />
    </label>
    <input
      type="text"
      list="okf-stereotypes"
      value={node.stereotypes.join(", ")}
      oninput={(e) => onUpdate({ stereotypes: e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean) })}
      placeholder="aggregateRoot, entity"
      class={inputCls}
    />
    <datalist id="okf-stereotypes">
      {#each palette.stereotypes as s (s)}
        <option value={s}></option>
      {/each}
    </datalist>
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
              class="w-full text-left text-[13px] text-slate-900 break-words flex items-baseline gap-[6px] rounded-md -mx-1 px-1 py-[2px] hover:bg-[#f1f3f7] focus:outline-none focus:ring-2 focus:ring-[#e6f1fb]"
            >
              <span class="text-slate-400 font-mono">{a.outgoing ? "→" : "←"}</span>
              <span class="font-semibold">{a.otherTitle}</span>
              <span class="text-[11px] text-slate-500">{a.kind}{a.role ? ` (${a.role})` : ""}{a.multiplicity ? ` [${a.multiplicity}]` : ""}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <div class="text-[13px] text-slate-400 italic">No associations</div>
    {/if}
  </div>
  {#if isEnum}
    <div>
      <label class={labelCls} for="oi-values">Values (one per line)</label>
      <textarea
        id="oi-values"
        value={(node.values ?? []).join("\n")}
        rows={5}
        oninput={(e) => onUpdate({ values: e.currentTarget.value.split("\n").map((v) => v.trim()).filter(Boolean) })}
        class={`${inputCls} font-mono resize-y`}
      ></textarea>
    </div>
  {:else}
    <div>
      <span class={labelCls}>Attributes</span>
      <AttributeEditor attributes={node.attributes} onChange={(attributes: Attribute[]) => onUpdate({ attributes })} />
    </div>
  {/if}
</div>
