<script lang="ts">
  // Mirrors packages/web/src/components/canvas/Canvas.tsx's `CanvasApp` — a thin
  // provider wrapper so CanvasInner's `useSvelteFlow()` (screenToFlowPosition,
  // fitView) has flow context to attach to.
  import { SvelteFlowProvider } from "@xyflow/svelte";
  import CanvasInner from "./CanvasInner.svelte";
</script>

<SvelteFlowProvider>
  <CanvasInner />
</SvelteFlowProvider>
