<script lang="ts">
  // Floating toolbar anchored above the selection's bounding box. Presentational:
  // the parent computes the screen anchor (x,y — top-center of the box) and the
  // selected node/edge counts, and supplies the action callbacks.
  import { Trash2, LayoutDashboard } from "lucide-svelte";

  let {
    x,
    y,
    nodeCount,
    edgeCount,
    onNewDiagram,
    onDelete,
  }: {
    /** Screen x of the selection box's top-center (client coords, position:fixed). */
    x: number;
    /** Screen y of the selection box's top edge (client coords). */
    y: number;
    nodeCount: number;
    edgeCount: number;
    onNewDiagram: (name: string) => void;
    onDelete: () => void;
  } = $props();

  // Inline-name mode for "New diagram from selection" (never window.prompt).
  let naming = $state(false);
  let name = $state("");

  const total = $derived(nodeCount + edgeCount);
  // Mixed / edges-only selection: a diagram needs at least one node member.
  const canCreate = $derived(nodeCount > 0);
  const summary = $derived(total === 1 ? "1 selected" : `${total} selected`);

  function startNaming() {
    naming = true;
    name = "";
  }
  function confirm() {
    const t = name.trim();
    if (!t) return; // reject empty / whitespace
    onNewDiagram(t);
    naming = false;
    name = "";
  }
  function cancel() {
    naming = false;
    name = "";
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      confirm();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    }
  }
</script>

<!-- Positioned above the selection (translate(-50%,-100%)) so it never covers the
     elements it acts on. Fixed → the parent's client coords map straight through.
     `nopan`/`nodrag` keep clicks from reaching the canvas underneath. -->
<div
  data-testid="selection-toolbar"
  class="nopan nodrag fixed z-30 -translate-x-1/2 -translate-y-full"
  style="left:{x}px; top:{y}px; margin-top:-10px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
>
  <div
    class="flex items-center gap-1 rounded-xl border border-[#d8dee8] bg-white p-[6px] shadow-[0_8px_24px_rgba(15,23,42,0.14)]"
  >
    {#if naming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        aria-label="New diagram name"
        bind:value={name}
        onkeydown={onKey}
        placeholder="New diagram name"
        autofocus
        class="w-[180px] text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
      />
      <button
        onclick={confirm}
        aria-label="Create diagram"
        class="rounded-lg bg-[#1e88e5] px-3 py-[7px] text-[12px] font-semibold text-white hover:bg-[#1976d2] whitespace-nowrap"
      >
        Create diagram
      </button>
      <button
        onclick={cancel}
        aria-label="Cancel"
        class="rounded-lg px-2 py-[7px] text-[12px] font-medium text-slate-500 hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
    {:else}
      <span class="px-2 text-[12px] font-medium text-slate-500 whitespace-nowrap">{summary}</span>
      <div class="h-[20px] w-px bg-[#e2e6ec]"></div>
      <button
        onclick={startNaming}
        disabled={!canCreate}
        aria-label="New diagram from selection"
        title={canCreate
          ? "New diagram seeded with the selected objects"
          : "Select at least one object to create a diagram"}
        class="flex items-center gap-[6px] rounded-lg px-2.5 py-[7px] text-[12px] font-semibold whitespace-nowrap transition-colors {canCreate
          ? 'text-[#1e88e5] hover:bg-[#e6f1fb] cursor-pointer'
          : 'text-slate-300 cursor-not-allowed'}"
      >
        <LayoutDashboard size={14} /> New diagram from selection
      </button>
      <button
        onclick={onDelete}
        aria-label="Delete selection"
        title="Delete the selected objects and relationships"
        class="flex items-center gap-[6px] rounded-lg px-2.5 py-[7px] text-[12px] font-semibold text-slate-500 hover:bg-[#fdf2f2] hover:text-[#dc2626] cursor-pointer whitespace-nowrap transition-colors"
      >
        <Trash2 size={14} /> Delete selection
      </button>
    {/if}
  </div>
</div>
