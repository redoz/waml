<!-- packages/web/src/components/central/ElementPreview.svelte -->
<script lang="ts">
	// static, live-updating cropped render edited element in context.
	// No pan/zoom/click/drag — purely view. Wraps its own SvelteFlowProvider so
	// its flow context isolated real canvas behind dialog.
	import { SvelteFlowProvider } from "@xyflow/svelte";
	import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
	import ElementPreviewCanvas from "./ElementPreviewCanvas.svelte";
	import { nodePreviewSubset, edgePreviewSubset } from "./previewSubset";

	let { mode, focalKey, nodes, edges, display, profileName }: {
		mode: "node" | "edge";
		focalKey: string;
		nodes: ModelNode[];
		edges: ModelEdge[];
		display: DiagramDisplay;
		profileName: string;
	} = $props();

	const subset = $derived(
		mode === "node"
			? nodePreviewSubset(focalKey, nodes, edges)
			: edgePreviewSubset(focalKey, nodes, edges),
	);
</script>

<div
	class="h-[220px] shrink-0 border-b border-[#d8dee8] bg-[#f7f8fa]"
	data-testid="element-preview"
>
	<SvelteFlowProvider>
		<ElementPreviewCanvas
			nodes={subset.nodes}
			edges={subset.edges}
			focalKeys={subset.focalKeys}
			{display}
			{profileName}
		/>
	</SvelteFlowProvider>
</div>
