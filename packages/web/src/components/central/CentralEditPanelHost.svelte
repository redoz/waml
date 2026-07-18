<script module lang="ts">
  // What the central panel is currently editing. `null` means the panel is
  // closed. An element edits one model node's fields; an edge edits one model
  // relationship's fields; a diagram edits the active diagram's display settings.
  export type CentralPanelState =
    | { kind: "element"; nodeKey: string }
    | { kind: "edge"; edgeKey: string }
    | { kind: "diagram" };
</script>

<script lang="ts">
  import type { DiagramDisplay, ModelNode, ModelEdge, Diagram } from "@waml/okf";
  import CentralEditPanel from "./CentralEditPanel.svelte";
  import ObjectInspector from "../inspector/ObjectInspector.svelte";
  import RelationshipInspector from "../inspector/RelationshipInspector.svelte";
  import DiagramPropertiesBody from "../canvas/DiagramPropertiesBody.svelte";
  import ElementPicker, { type Kind, KIND_ICON } from "../inspector/ElementPicker.svelte";

  let {
    state,
    nodes,
    edges,
    display,
    diagram,
    candidateStereotypes,
    editable,
    profileName,
    options,
    showPreview = false,
    previewEl = $bindable(null),
    onSelectElement,
    onUpdateNode,
    onUpdateEdge,
    onDisplayChange,
    onUpdateDiagram,
    onClose,
  }: {
    state: CentralPanelState | null;
    nodes: ModelNode[];
    edges: ModelEdge[];
    display: DiagramDisplay;
    diagram: Diagram;
    candidateStereotypes: string[];
    editable: boolean;
    profileName?: string;
    /** Element-picker entries (diagram + objects + associations), shared with
     *  the floating inspector so the dialog header switches what it edits. */
    options: { key: string; label: string; kind: Kind }[];
    /** Cut a transparent hole above the fields so the live canvas behind the
     *  dialog shows through it. Omit when there is no active diagram behind
     *  the dialog (Navigator's out-of-diagram context). */
    showPreview?: boolean;
    /** The cutout's DOM element, bound up to the caller so it can compute the
     *  viewport transform that frames the focal node/edge inside it. */
    previewEl?: HTMLDivElement | null;
    /** Repoint the dialog at another element/edge/diagram — the header picker
     *  and clicked associations both route through this. */
    onSelectElement: (key: string, kind: Kind) => void;
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onUpdateEdge: (id: string, patch: Partial<ModelEdge>) => void;
    onDisplayChange: (patch: Partial<DiagramDisplay>) => void;
    onUpdateDiagram: (patch: Partial<Diagram>) => void;
    onClose: () => void;
  } = $props();

  // Resolve the edited node/edge; a since-deleted key resolves to undefined.
  const node = $derived(
    state?.kind === "element" ? nodes.find((n) => n.key === state.nodeKey) : undefined,
  );
  const edge = $derived(
    state?.kind === "edge" ? edges.find((e) => e.id === state.edgeKey) : undefined,
  );

  // The picker's current selection, mapped from what the dialog edits.
  const selectedKey = $derived(
    state?.kind === "element" ? state.nodeKey
      : state?.kind === "edge" ? state.edgeKey
      : state?.kind === "diagram" ? diagram.key
      : null,
  );
  const focusedKind = $derived<Kind | undefined>(
    state?.kind === "element" ? "node" : state?.kind === "edge" ? "edge" : state?.kind === "diagram" ? "diagram" : undefined,
  );

  // Pointing at a since-deleted key: close instead of showing an empty shell.
  $effect(() => {
    if (state?.kind === "element" && !node) onClose();
  });
  $effect(() => {
    if (state?.kind === "edge" && !edge) onClose();
  });
</script>

<!-- Dialog header: the same element picker the floating inspector uses, so the
     dialog can switch what it edits without closing. A kind badge fronts it. -->
{#snippet pickerHeader()}
  <div class="flex items-center gap-2 min-w-0">
    {#if focusedKind}
      {@const KindIcon = KIND_ICON[focusedKind]}
      <span class="flex-none w-[26px] h-[26px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--accent))] bg-[color:rgba(var(--accent),.12)]">
        <KindIcon size={15} />
      </span>
    {/if}
    <div class="flex-1 min-w-0">
      <ElementPicker {options} {selectedKey} onSelect={onSelectElement} />
    </div>
  </div>
{/snippet}

{#if state?.kind === "element" && node}
  <CentralEditPanel title={node.concept.title?.trim() || "Untitled"} header={pickerHeader} fullHeight {showPreview} bind:previewEl {onClose}>
    <ObjectInspector
      {node}
      {nodes}
      {edges}
      onUpdate={(patch) => onUpdateNode(node.key, patch)}
      onSelectAssociation={(id) => onSelectElement(id, "edge")}
      {profileName}
    />
  </CentralEditPanel>
{:else if state?.kind === "edge" && edge}
  <CentralEditPanel title="Relationship" header={pickerHeader} fullHeight {showPreview} bind:previewEl {onClose}>
    <RelationshipInspector
      {edge}
      fromNode={nodes.find((n) => n.key === edge.from)}
      toNode={nodes.find((n) => n.key === edge.to)}
      onUpdate={(patch) => onUpdateEdge(edge.id, patch)}
    />
  </CentralEditPanel>
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" header={pickerHeader} fullHeight {onClose}>
    <DiagramPropertiesBody
      {display} {diagram} {candidateStereotypes} {editable}
      onChange={onDisplayChange}
      {onUpdateDiagram}
    />
  </CentralEditPanel>
{/if}
