<script lang="ts">
  import { GripVertical } from "lucide-svelte";
  import type { Attribute, Visibility } from "@waml/okf";
  import InfoTip from "./InfoTip.svelte";

  const VISIBILITIES: (Visibility | "")[] = ["", "+", "-", "#", "~"];

  let { attributes, onChange }: {
    attributes: Attribute[];
    onChange: (attributes: Attribute[]) => void;
  } = $props();

  let dragIdx = $state<number | null>(null);
  let overIdx = $state<number | null>(null);

  const update = (i: number, patch: Partial<Attribute>) =>
    onChange(attributes.map((a, idx) => (idx === i ? { ...a, ...patch } : a)));
  const remove = (i: number) => onChange(attributes.filter((_, idx) => idx !== i));
  const add = () => onChange([...attributes, { name: "", type: { name: "String" }, multiplicity: "1" }]);
  const move = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0) return;
    const next = attributes.slice();
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    onChange(next);
  };

  const cols = "16px minmax(100px,1fr) minmax(90px,1fr) 62px 52px minmax(120px,1.3fr) 24px";
  const inputCls = "w-full text-[12.5px] px-[7px] py-[5px] border border-[color:var(--hair)] rounded-lg text-slate-900 focus:outline-none focus:border-[color:rgb(var(--accent))] focus:ring-2 focus:ring-[color:rgba(var(--accent),.20)]";
</script>

<div class="border border-[color:var(--hair)] rounded-[10px] overflow-hidden">
  <div class="overflow-x-auto">
    <div class="min-w-[540px]">
      <div
        class="grid bg-[color:rgba(var(--accent),.04)] px-[10px] py-[7px] text-[10.5px] font-semibold text-slate-500 uppercase tracking-[0.3px] border-b border-[color:var(--hair)] gap-[6px]"
        style={`grid-template-columns: ${cols}`}
      >
        <span></span>
        <span>Name</span>
        <span class="flex items-center gap-[3px]"
          >Type <InfoTip
            text="A bare token (String, OrderId) or another classifier's title. Links to other docs survive import; editing the text keeps a plain token."
          /></span
        >
        <span class="flex items-center gap-[3px]"
          >Mult <InfoTip text="UML multiplicity: 1, 0..1, *, 1..*, 2..5. Blank means 1." /></span
        >
        <span class="flex items-center gap-[3px]"
          >Vis <InfoTip
            text="Visibility: + public, - private, # protected, ~ package. Optional; the uml-domain profile hides it on canvas."
          /></span
        >
        <span>Description</span>
        <span></span>
      </div>
      {#each attributes as a, i (i)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          ondragover={(e) => {
            if (dragIdx === null) return;
            e.preventDefault();
            if (overIdx !== i) overIdx = i;
          }}
          ondrop={(e) => {
            e.preventDefault();
            if (dragIdx !== null) move(dragIdx, i);
            dragIdx = null;
            overIdx = null;
          }}
          class={`grid px-[10px] py-[6px] border-b border-[color:var(--hair)] last:border-b-0 items-center gap-[6px] ${dragIdx === i ? "opacity-40" : ""} ${overIdx === i && dragIdx !== null && dragIdx !== i ? "bg-[color:rgba(var(--accent),.10)]" : ""}`}
          style={`grid-template-columns: ${cols}`}
        >
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            draggable={true}
            ondragstart={(e) => {
              dragIdx = i;
              if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
            }}
            ondragend={() => {
              dragIdx = null;
              overIdx = null;
            }}
            title="Drag to reorder"
            class="flex items-center justify-center text-slate-300 hover:text-slate-500 cursor-grab active:cursor-grabbing"
          >
            <GripVertical size={13} />
          </span>
          <input
            type="text"
            value={a.name}
            placeholder="name"
            oninput={(e) => update(i, { name: e.currentTarget.value })}
            class={inputCls}
          />
          <input
            type="text"
            value={a.type.name}
            placeholder="String"
            oninput={(e) => update(i, { type: { name: e.currentTarget.value } })}
            class={inputCls}
          />
          <input
            type="text"
            value={a.multiplicity}
            placeholder="1"
            oninput={(e) => update(i, { multiplicity: e.currentTarget.value || "1" })}
            class={inputCls}
          />
          <select
            value={a.visibility ?? ""}
            aria-label="Visibility"
            onchange={(e) => update(i, { visibility: (e.currentTarget.value || undefined) as Visibility | undefined })}
            class="w-full text-[11.5px] px-[4px] py-[5px] border border-[color:var(--hair)] rounded-lg text-slate-900"
          >
            {#each VISIBILITIES as v (v)}
              <option value={v}>{v || "—"}</option>
            {/each}
          </select>
          <input
            type="text"
            value={a.description ?? ""}
            placeholder="description"
            oninput={(e) => update(i, { description: e.currentTarget.value || undefined })}
            class={inputCls}
          />
          <button
            onclick={() => remove(i)}
            title="Remove attribute"
            class="border-none bg-transparent text-slate-300 cursor-pointer text-[15px] p-0 hover:text-[color:rgb(var(--danger))] flex items-center justify-center"
          >
            &times;
          </button>
        </div>
      {/each}
    </div>
  </div>
  <button
    onclick={add}
    class="w-full border-none bg-[color:var(--panel-fill)] px-2 py-[8px] text-[12.5px] font-semibold text-[color:rgb(var(--accent))] cursor-pointer hover:bg-[color:rgba(var(--accent),.06)] transition-colors border-t border-[color:var(--hair)]"
  >
    + Add attribute
  </button>
</div>
