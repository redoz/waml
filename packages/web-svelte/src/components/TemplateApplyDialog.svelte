<script lang="ts">
  // TODO(plan-3b): port TemplateApplyDialog.tsx here. Props contract is frozen.
  import type { ModelGraph } from "@mc/okf";

  let { graph, name, onConfirm, onClose }: {
    graph: ModelGraph;
    name: string;
    onConfirm: (mode: "replace" | "merge") => void;
    onClose: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): TemplateApplyDialog markup -->
