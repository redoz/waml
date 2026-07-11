<script lang="ts">
  // TODO(plan-3b): port WelcomeDialog.tsx here. Props contract is frozen.
  import type { ModelGraph } from "@mc/okf";

  let { onUseTemplate, onStartBlank, onImport }: {
    onUseTemplate: (g: ModelGraph, name: string) => void;
    onStartBlank: () => void;
    onImport: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): WelcomeDialog markup -->
