<script module lang="ts">
  // What the central panel is currently editing. `null` means the panel is
  // closed. An element edits one model node's fields; a diagram edits the active
  // diagram's display settings.
  export type CentralPanelState =
    | { kind: "element"; nodeKey: string }
    | { kind: "diagram" };
</script>

<script lang="ts">
  import type { DiagramDisplay, ModelNode } from "@waml/okf";
  import CentralEditPanel from "./CentralEditPanel.svelte";
  import ObjectInspector from "../inspector/ObjectInspector.svelte";
  import DiagramPropertiesBody from "../canvas/DiagramPropertiesBody.svelte";

  let { state, nodes, display, profileName, onUpdateNode, onDisplayChange, onClose }: {
    state: CentralPanelState | null;
    nodes: ModelNode[];
    display: DiagramDisplay;
    profileName?: string;
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onDisplayChange: (patch: Partial<DiagramDisplay>) => void;
    onClose: () => void;
  } = $props();

  // Resolve the edited node (element context only); a since-deleted key resolves
  // to undefined, mirroring today's `focused` guard.
  const node = $derived(
    state?.kind === "element" ? nodes.find((n) => n.key === state.nodeKey) : undefined,
  );

  // Element pointing at a since-deleted key: close instead of showing an empty
  // shell. Runs as an effect so it fires on the offending render.
  $effect(() => {
    if (state?.kind === "element" && !node) onClose();
  });
</script>

{#if state?.kind === "element" && node}
  <CentralEditPanel title={node.concept.title?.trim() || "Untitled"} {onClose}>
    <ObjectInspector
      {node}
      onUpdate={(patch) => onUpdateNode(node.key, patch)}
      {profileName}
    />
  </CentralEditPanel>
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" {onClose}>
    <DiagramPropertiesBody {display} onChange={onDisplayChange} />
  </CentralEditPanel>
{/if}
