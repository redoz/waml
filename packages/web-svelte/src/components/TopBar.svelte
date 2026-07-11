<script lang="ts">
  // TODO(plan-3b): port TopBar.tsx here. Props contract is frozen.
  let { onImport, onExport, onExportSvg, exportDisabled, onShare, shareDisabled, onLibrary, onOpenGoal, goalSet }: {
    onImport: () => void;
    onExport: () => void;
    onExportSvg: () => void;
    exportDisabled: boolean;
    onShare: () => void;
    shareDisabled: boolean;
    onLibrary: () => void;
    onOpenGoal: () => void;
    goalSet: boolean;
  } = $props();
</script>

<!-- TODO(plan-3b): TopBar markup -->
