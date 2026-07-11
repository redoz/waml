<script lang="ts">
  // TODO(plan-3b): port ModelSheet.tsx here. Props contract is frozen.
  import type { Snippet } from "svelte";
  import type { RightPanelId } from "./rightPanel.svelte";

  let { active, modal, title, onClose, children }: {
    active: RightPanelId | null;
    modal: boolean;
    title: string;
    onClose: () => void;
    children: Snippet;
  } = $props();
</script>

<!-- TODO(plan-3b): ModelSheet markup -->
