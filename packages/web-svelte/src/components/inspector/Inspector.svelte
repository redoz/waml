<script lang="ts">
  // Mirrors packages/web/src/components/inspector/Inspector.tsx.
  import type { Snippet } from "svelte";
  import { PanelRightOpen } from "lucide-svelte";
  import type { ModelNode, ModelEdge } from "@mc/okf";
  import type { Selection } from "../canvas/selection";
  import ObjectInspector from "./ObjectInspector.svelte";
  import RelationshipInspector from "./RelationshipInspector.svelte";

  const MIN_WIDTH = 320;

  let {
    selection, nodes, edges, onUpdateNode, onUpdateEdge, onClose, embedded = false, profileName, externalRefs,
  }: {
    selection: Selection;
    nodes: ModelNode[];
    edges: ModelEdge[];
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onUpdateEdge: (id: string, patch: Partial<ModelEdge>) => void;
    onClose: () => void;
    profileName?: string;
    externalRefs?: Snippet;
    embedded?: boolean;
  } = $props();

  let open = $state(true);
  let width = $state(320);
  let resizing = false;
  let startX = 0;
  let startWidth = 0;

  const selectedNode = $derived(
    selection?.type === "node" ? nodes.find(n => n.key === selection.id) : undefined,
  );
  const selectedEdge = $derived(
    selection?.type === "edge" ? edges.find(e => e.id === selection.id) : undefined,
  );

  const title = $derived(
    selectedNode ? (selectedNode.title.trim() || "Untitled")
      : selectedEdge ? "Relationship" : "Inspector",
  );

  // Resize drag handlers
  function onResizeMouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    startX = e.clientX;
    startWidth = width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  $effect(() => {
    function onMouseMove(e: MouseEvent) {
      if (!resizing) return;
      const delta = startX - e.clientX;
      width = Math.min(window.innerWidth * 0.5, Math.max(MIN_WIDTH, startWidth + delta));
    }
    function onMouseUp() {
      if (!resizing) return;
      resizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    }
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  });
</script>

{#snippet emptyState()}
  <div class="px-6 py-[46px] text-center text-slate-500 text-[13px] leading-[1.6]">
    <svg
      viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={1.5}
      width={42} height={42}
      class="mx-auto mb-3 opacity-35"
    >
      <rect x="3" y="4" width="7" height="6" rx="1.5" />
      <rect x="14" y="4" width="7" height="6" rx="1.5" />
      <rect x="9" y="14" width="7" height="6" rx="1.5" />
    </svg>
    <div>
      Select an object or relationship to edit.
      <br /><br />
      Changes apply to your local model.
    </div>
  </div>
{/snippet}

{#snippet reopenTab(onClick: () => void)}
  <button
    onclick={onClick}
    title="Open inspector"
    aria-label="Open inspector"
    class="group absolute right-0 top-1/2 -translate-y-1/2 z-20 flex h-[46px] w-[32px] items-center justify-center rounded-l-xl border border-r-0 border-[#d8dee8] bg-white text-slate-500 shadow-[-3px_0_12px_rgba(15,23,42,0.07)] cursor-pointer transition-colors hover:bg-[#f1f3f7] hover:text-[#1e88e5]"
  >
    <PanelRightOpen size={18} />
    <span class="pointer-events-none absolute right-[calc(100%+8px)] top-1/2 -translate-y-1/2 whitespace-nowrap rounded-md bg-slate-900 px-2 py-1 text-[12px] font-medium text-white opacity-0 transition-opacity group-hover:opacity-100 shadow-[0_6px_18px_rgba(15,23,42,0.28)]">
      Open inspector
    </span>
  </button>
{/snippet}

{#snippet body()}
  {#if selectedNode}
    <ObjectInspector
      node={selectedNode}
      onUpdate={(patch) => onUpdateNode(selectedNode.key, patch)}
      profileName={profileName}
    />
    {@render externalRefs?.()}
  {:else if selectedEdge}
    <RelationshipInspector
      edge={selectedEdge}
      fromNode={nodes.find(n => n.key === selectedEdge.from)}
      toNode={nodes.find(n => n.key === selectedEdge.to)}
      onUpdate={(patch) => onUpdateEdge(selectedEdge.id, patch)}
    />
  {:else}
    {@render emptyState()}
  {/if}
{/snippet}

{#if embedded}
  {@render body()}
{:else if !open}
  <div class="relative flex-shrink-0" style="width: 0">
    {@render reopenTab(() => { open = true; })}
  </div>
{:else}
  <div
    class="bg-white border-l border-[#d8dee8] flex-shrink-0 flex flex-col z-10 shadow-[-4px_0_16px_rgba(15,23,42,0.04)] relative"
    style={`width: ${width}px; min-width: ${MIN_WIDTH}px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;`}
  >
    <!-- Resize handle -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onmousedown={onResizeMouseDown}
      class="absolute left-0 top-0 w-[7px] h-full cursor-col-resize z-[18] group"
      title="Drag to resize"
    >
      <div class="absolute left-[2px] top-0 w-[2px] h-full bg-transparent group-hover:bg-[#1e88e5] transition-colors"></div>
    </div>

    <!-- Header -->
    <div class="px-4 py-[14px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0">
      <h3 class="text-[13.5px] font-[650] flex-1 text-slate-900">{title}</h3>
      <button
        onclick={() => { onClose(); open = false; }}
        title="Close inspector"
        class="cursor-pointer text-slate-500 border-none bg-none text-[18px] leading-none hover:text-slate-900 transition-colors p-0 bg-transparent"
      >
        ×
      </button>
    </div>

    <!-- Body -->
    <div class="px-4 py-4 overflow-y-auto flex-1 min-h-0">
      {@render body()}
    </div>
  </div>
{/if}
