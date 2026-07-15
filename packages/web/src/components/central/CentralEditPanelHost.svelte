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
  import ElementPreview from "./ElementPreview.svelte";
  import ObjectInspector from "../inspector/ObjectInspector.svelte";
  import RelationshipInspector from "../inspector/RelationshipInspector.svelte";
  import DiagramPropertiesBody from "../canvas/DiagramPropertiesBody.svelte";

  let {
    state,
    nodes,
    edges,
    display,
    diagram,
    candidateStereotypes,
    editable,
    profileName,
    showPreview = false,
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
    /** Render the live cropped preview above the fields. Omit when there is no
     *  active diagram behind the dialog (Navigator's out-of-diagram context). */
    showPreview?: boolean;
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

  // Pointing at a since-deleted key: close instead of showing an empty shell.
  $effect(() => {
    if (state?.kind === "element" && !node) onClose();
  });
  $effect(() => {
    if (state?.kind === "edge" && !edge) onClose();
  });
</script>

{#if state?.kind === "element" && node}
  <CentralEditPanel title={node.concept.title?.trim() || "Untitled"} fullHeight {onClose}>
    {#snippet preview()}
      {#if showPreview}
        <ElementPreview mode="node" focalKey={node.key} {nodes} {edges} {display} profileName={profileName ?? ""} />
      {/if}
    {/snippet}
    <ObjectInspector
      {node}
      onUpdate={(patch) => onUpdateNode(node.key, patch)}
      {profileName}
    />
  </CentralEditPanel>
{:else if state?.kind === "edge" && edge}
  <CentralEditPanel title="Relationship" fullHeight {onClose}>
    {#snippet preview()}
      {#if showPreview}
        <ElementPreview mode="edge" focalKey={edge.id} {nodes} {edges} {display} profileName={profileName ?? ""} />
      {/if}
    {/snippet}
    <RelationshipInspector
      {edge}
      fromNode={nodes.find((n) => n.key === edge.from)}
      toNode={nodes.find((n) => n.key === edge.to)}
      onUpdate={(patch) => onUpdateEdge(edge.id, patch)}
    />
  </CentralEditPanel>
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" {onClose}>
    <DiagramPropertiesBody
      {display} {diagram} {candidateStereotypes} {editable}
      onChange={onDisplayChange}
      {onUpdateDiagram}
    />
  </CentralEditPanel>
{/if}
