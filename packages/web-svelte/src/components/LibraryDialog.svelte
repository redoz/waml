<script lang="ts">
  // TODO(plan-3b): port LibraryDialog.tsx here. Props contract is frozen.
  import type { ModelGraph } from "@mc/okf";

  let { onUse, onClose }: {
    onUse: (g: ModelGraph, name: string) => void;
    onClose: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): LibraryDialog markup -->
