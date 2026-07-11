<script lang="ts">
  // TODO(plan-3b): port GoalDialog.tsx here. Props contract is frozen.
  import type { BusinessGoal } from "@mc/core/state/goal";

  let { current, onConfirm, onClear, onClose }: {
    current: BusinessGoal | null;
    onConfirm: (g: BusinessGoal) => void;
    onClear: () => void;
    onClose: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): GoalDialog markup -->
