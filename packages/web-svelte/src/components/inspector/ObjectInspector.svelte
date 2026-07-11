<script lang="ts">
  import type { ModelNode, Attribute } from "@mc/okf";
  import AttributeEditor from "./AttributeEditor.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { getProfile } from "@mc/core/profiles";

  let { node, onUpdate, profileName }: {
    node: ModelNode;
    onUpdate: (patch: Partial<ModelNode>) => void;
    profileName?: string;
  } = $props();

  const inputCls = "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";
  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";

  let palette = $derived(getProfile(profileName).palette);
  let isEnum = $derived(node.type === "uml.Enum");
</script>

<div class="flex flex-col gap-[15px]">
  <div>
    <label class={labelCls} for="oi-title">Title</label>
    <input id="oi-title" type="text" value={node.title} oninput={(e) => onUpdate({ title: e.currentTarget.value })} class={inputCls} />
  </div>
  <div>
    <label class={labelCls} for="oi-description">Description</label>
    <textarea
      id="oi-description"
      value={node.description ?? ""}
      rows={3}
      oninput={(e) => onUpdate({ description: e.currentTarget.value || undefined })}
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
