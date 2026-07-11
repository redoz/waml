<script lang="ts">
  // TODO(plan-3b): port ImportDialog.tsx here. Props contract is frozen.
  import type { ModelGraph } from "@mc/okf";

  let { onConfirm, onClose }: {
    onConfirm: (g: ModelGraph, mode: "replace" | "merge") => void;
    onClose: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): ImportDialog markup -->
