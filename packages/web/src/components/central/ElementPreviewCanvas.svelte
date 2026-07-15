<!-- packages/web/src/components/central/ElementPreviewCanvas.svelte -->
<script lang="ts">
	// read-only inner flow. Must be mounted inside SvelteFlowProvider (see
	// ElementPreview.svelte) so useSvelteFlow()/fitView bind to an ISOLATED flow
	// context, never real canvas's.
	import { SvelteFlow, useSvelteFlow, type Node, type Edge } from "@xyflow/svelte";
	import { tick } from "svelte";
	import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
	import { toRFNode } from "../canvas/toRFNode";
	import { buildRfEdges } from "../canvas/edges";
	import { nodeTypes, edgeTypes } from "../canvas/flowTypes";

	// Context neighbours + connecting edges render dimmed. Reuse app's
	// established opacity-40 dim convention (0.4) rather inventing value.
	const DIM = "opacity:0.4";

	let { nodes, edges, focalKeys, display, profileName }: {
		nodes: ModelNode[];
		edges: ModelEdge[];
		focalKeys: Set<string>;
		display: DiagramDisplay;
		profileName: string;
	} = $props();

	// SvelteFlow binds these; we rebuild them props on every subset change
	// (mirrors CanvasInner's rfNodes/rfEdges effect pattern).
	let rfNodes = $state<Node[]>([]);
	let rfEdges = $state<Edge[]>([]);

	$effect(() => {
		rfNodes = nodes.map((n) => ({
			...toRFNode(n, display, profileName),
			style: focalKeys.has(n.key) ? undefined : DIM,
		}));

		rfEdges = buildRfEdges(edges, nodes, display).map((e) => ({
			...e,
			style: focalKeys.has(e.source) && focalKeys.has(e.target) ? undefined : DIM,
		}));
	});

	const { fitView } = useSvelteFlow();

	// Re-crop rendered set on mount whenever geometry changes. Guard the
	// microtask continuation with a per-effect cancellation flag: if the
	// component/effect is destroyed (e.g. dialog closes) before tick()
	// resolves, calling fitView() would read a derived belonging to a
	// now-destroyed effect (Svelte's `derived_inert` warning).
	$effect(() => {
		void rfNodes;
		void rfEdges;
		let cancelled = false;
		tick().then(() => {
			if (!cancelled) fitView({ padding: 0.2, duration: 0 });
		});
		return () => {
			cancelled = true;
		};
	});
</script>

<SvelteFlow
	bind:nodes={rfNodes}
	bind:edges={rfEdges}
	{nodeTypes}
	{edgeTypes}
	fitView
	nodesDraggable={false}
	nodesConnectable={false}
	elementsSelectable={false}
	panOnDrag={false}
	panOnScroll={false}
	zoomOnScroll={false}
	zoomOnDoubleClick={false}
	minZoom={0.2}
	maxZoom={2}
/>
