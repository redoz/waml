<script lang="ts">
  // TODO(plan-3b): port ClearCanvasDialog.tsx here. Props contract is frozen.
  let { counts, onDelete, onExportAndDelete, onClose }: {
    counts: { marts: number; relationships: number };
    onDelete: () => void;
    onExportAndDelete: () => void;
    onClose: () => void;
  } = $props();
</script>

<!-- TODO(plan-3b): ClearCanvasDialog markup -->
