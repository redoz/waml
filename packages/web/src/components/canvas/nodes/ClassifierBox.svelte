<script lang="ts">
  import type { Snippet } from "svelte";
  import { getProfile, stereotypeStyle } from "@uaml/core/profiles";
  import { resolveDisplay } from "@uaml/okf";
  import NodePorts from "./NodePorts.svelte";
  import StereotypeRow from "./StereotypeRow.svelte";
  import AttributeRow from "./AttributeRow.svelte";
  import RowsCompartment from "./RowsCompartment.svelte";
  import { NODE_FONT, type OkfNodeData } from "./types";

  let { data, keyword, header }: { data: OkfNodeData; keyword?: string; header?: Snippet } = $props();

  let profile = $derived(getProfile(data._profile));
  let st = $derived(stereotypeStyle(profile, data.stereotypes));
  // Per-diagram render settings (resolved: absent ⇒ defaults).
  let display = $derived(resolveDisplay(data._display));
  let isDetailed = $derived(display.showAttributes);
  let showTypes = $derived(display.attributeDetail === "name-type");
  let showStereotype = $derived(display.showStereotype);
  let showVisibility = $derived(!profile.hide.includes("visibility"));
  let hasStereotypeStyle = $derived(Object.keys(st).length > 0);

  let boxStyle = $derived.by(() => {
    const decls: string[] = [`font-family:${NODE_FONT}`];
    if (st.header) decls.push(`border-top-color:${st.header}`, `border-top-width:4px`);
    if (st.border === "thick") decls.push(`border-color:${st.header ?? "#334155"}`, `border-width:2.5px`);
    if (st.shape === "hexagon") {
      decls.push(`clip-path:polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)`, `border-radius:0`);
    }
    return decls.join(";");
  });
</script>

<div
  data-stereotyped={hasStereotypeStyle ? true : undefined}
  class="relative bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-[0_2px_8px_rgba(15,23,42,0.05)] cursor-grab hover:border-[#c2cad8] select-none w-[230px]"
  style={boxStyle}
>
  <NodePorts />
  <div class="relative z-[1]">
    {@render header?.()}
    {#if showStereotype}
      <StereotypeRow stereotypes={data.stereotypes} {keyword} />
    {/if}
    <div class={`px-3 pb-[9px] pt-[3px] text-center text-[13.5px] font-semibold text-slate-900 ${data.abstract ? "italic" : ""}`}>
      {data.concept.title ?? "Untitled"}
    </div>
    {#if isDetailed && data.values && data.values.length > 0}
      <RowsCompartment rows={data.values.length}>
        {#snippet render(i: number)}
          <div class="px-3 py-[5px] text-[11.5px] text-slate-800 border-b border-[#f3f5f8] last:border-b-0">
            {data.values?.[i]}
          </div>
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if isDetailed && !data.values}
      <RowsCompartment rows={data.attributes.length}>
        {#snippet render(i: number)}
          <AttributeRow a={data.attributes[i]} {showVisibility} {showTypes} />
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if !isDetailed}
      <div class="px-3 pb-[10px] text-center text-[11px] text-slate-500">
        {data.values ? `${data.values.length} values` : `${data.attributes.length} attribute${data.attributes.length === 1 ? "" : "s"}`}
      </div>
    {/if}
  </div>
</div>
