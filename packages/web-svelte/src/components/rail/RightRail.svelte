<script lang="ts">
  // TODO(plan-3b): port RightRail.tsx here. Props contract is frozen.
  import type { RightPanelId } from "./rightPanel.svelte";

  let { active, onOpen }: {
    active: RightPanelId | null;
    onOpen: (id: RightPanelId) => void;
  } = $props();
</script>

<!-- TODO(plan-3b): RightRail markup -->
