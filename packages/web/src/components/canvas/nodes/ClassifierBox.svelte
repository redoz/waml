<script lang="ts">
  import type { Snippet } from "svelte";
  import { getProfile, stereotypeStyle } from "@waml/core/profiles";
  import { resolveDisplay } from "@waml/okf";
  import NodePorts from "./NodePorts.svelte";
  import StereotypeRow from "./StereotypeRow.svelte";
  import AttributeRow from "./AttributeRow.svelte";
  import RowsCompartment from "./RowsCompartment.svelte";
  import { hexToTriple, type OkfNodeData } from "./types";

  let { data, keyword, header }: { data: OkfNodeData; keyword?: string; header?: Snippet } = $props();

  let profile = $derived(getProfile(data._profile));
  let st = $derived(stereotypeStyle(profile, data.stereotypes));
  // Per-diagram render settings (resolved: absent ⇒ defaults).
  let display = $derived(resolveDisplay(data._display));
  let isDetailed = $derived(display.showAttributes);
  let showTypes = $derived(display.showType);
  let showStereotype = $derived(display.showStereotype);
  let showVisibility = $derived(!profile.hide.includes("visibility") && display.showAttributeVisibility);
  let hasStereotypeStyle = $derived(Object.keys(st).length > 0);
  let stereotypeTags = $derived(
    display.stereotypeFilter === undefined
      ? data.stereotypes
      : data.stereotypes.filter((s) => display.stereotypeFilter!.includes(s)),
  );

  let overrideHeader = $derived(
    data.stereotypes.reduce<string | undefined>((acc, s) => display.stereotypeColors[s] ?? acc, undefined),
  );
  let headerColor = $derived(overrideHeader ?? st.header);
  let accentTriple = $derived(hexToTriple(headerColor));

  // Structural per-node style: self-theme the accent triple; thick border and
  // hexagon shape stay as inline structural declarations (not Tailwind).
  let boxStyle = $derived.by(() => {
    const decls: string[] = [`--accent:${accentTriple}`];
    if (st.border === "thick") decls.push(`--bw:2.5px`);
    if (st.shape === "hexagon") {
      decls.push(`clip-path:polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)`);
    }
    return decls.join(";");
  });
</script>

<div
  data-stereotyped={hasStereotypeStyle ? true : undefined}
  class="hud-surface hud-surface--node hud-node"
  style={boxStyle}
>
  <NodePorts />
  <div class="hud-node__body">
    {@render header?.()}
    <div class={`node-hdr ${headerColor ? "node-hdr--fill" : "node-hdr--band"}`}>
      {#if showStereotype}
        <StereotypeRow stereotypes={stereotypeTags} {keyword} />
      {/if}
      <div class={`node-name ${data.abstract ? "node-name--abstract" : ""}`}>
        {data.concept.title ?? "Untitled"}
      </div>
    </div>
    {#if isDetailed && data.values && data.values.length > 0}
      <RowsCompartment rows={data.values.length}>
        {#snippet render(i: number)}
          <div class="node-row"><span class="node-row__name">{data.values?.[i]}</span></div>
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if isDetailed && !data.values}
      <RowsCompartment rows={data.attributes.length} max={display.maxAttributes}>
        {#snippet render(i: number)}
          <AttributeRow a={data.attributes[i]} {showVisibility} {showTypes} showMultiplicity={display.showAttributeMultiplicity} />
        {/snippet}
      </RowsCompartment>
    {/if}
    {#if !isDetailed}
      <div class="node-summary">
        {data.values ? `${data.values.length} values` : `${data.attributes.length} attribute${data.attributes.length === 1 ? "" : "s"}`}
      </div>
    {/if}
  </div>
</div>
