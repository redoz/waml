<script lang="ts">
  // The navigator sheet — a prop-driven presentational tree grown from the
  // TopBar switcher. All mutations are callbacks so it unit-tests like TopBar.
  import { Check, ChevronDown, Folder, FileText, StickyNote, Box } from "lucide-svelte";
  import { buildNavTree, type NavRow, type NavKind } from "@uaml/core/nav/tree";
  import { filterNav } from "@uaml/core/nav/search";
  import type { ModelGraph } from "@uaml/okf";

  let {
    graph,
    scopeKey = "",
    activeDiagramKey = "",
    palette = [],
    onScope,
    onSelectDiagram,
  }: {
    graph: ModelGraph;
    scopeKey?: string;
    activeDiagramKey?: string;
    palette?: string[];
    onScope?: (key: string) => void;
    onSelectDiagram?: (key: string) => void;
  } = $props();

  // Search box (filtering lands in Task 21; here it only toggles filterNav).
  let query = $state("");

  // The visible rows: filtered when searching, else the full scoped subtree.
  const rows = $derived<NavRow[]>(
    query ? filterNav(graph, scopeKey, query, "all").inScope : buildNavTree(graph, scopeKey),
  );

  // Breadcrumb: the root crumb (whole model) plus one crumb per scope segment,
  // each carrying its cumulative package key.
  const crumbs = $derived(
    [{ key: "", label: graph.path || "model" }].concat(
      scopeKey
        .split("/")
        .filter(Boolean)
        .map((seg, i, segs) => ({ key: segs.slice(0, i + 1).join("/"), label: seg })),
    ),
  );

  const KIND_ICON: Record<NavKind, typeof Folder> = {
    package: Folder,
    diagram: FileText,
    note: StickyNote,
    classifier: Box,
  };

  function activateRow(row: NavRow) {
    if (row.kind === "package") onScope?.(row.key);
    else if (row.kind === "diagram") onSelectDiagram?.(row.key);
  }
</script>

<div
  role="menu"
  tabindex="-1"
  class="w-[300px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1 text-[13px]"
>
  <!-- Search + type chip row -->
  <div class="flex items-center gap-1.5 px-2 py-1">
    <input
      aria-label="Search model"
      bind:value={query}
      placeholder="Search model"
      class="flex-1 min-w-0 px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
    />
    <button
      aria-label="Filter by type"
      class="flex items-center gap-[3px] px-2 py-[6px] rounded-md border border-[#d8dee8] text-slate-600 cursor-pointer hover:bg-[#f1f3f7]"
    >
      All <ChevronDown size={13} class="text-slate-400" />
    </button>
  </div>

  <!-- Scope breadcrumb -->
  <div class="flex items-center flex-wrap gap-[2px] px-3 py-1 text-[12px] text-slate-500">
    {#each crumbs as crumb, i (crumb.key)}
      {#if i > 0}<span class="text-slate-300">/</span>{/if}
      <button
        onclick={() => onScope?.(crumb.key)}
        class="px-1 rounded cursor-pointer hover:bg-[#f1f3f7] hover:text-slate-900"
      >
        {crumb.label}
      </button>
    {/each}
  </div>

  <div class="my-1 border-t border-[#eef1f5]"></div>

  <!-- Tree -->
  <div class="max-h-[420px] overflow-y-auto py-0.5">
    {#each rows as row (row.key)}
      {@const Icon = KIND_ICON[row.kind]}
      <button
        role="treeitem"
        onclick={() => activateRow(row)}
        style="padding-left:{8 + row.depth * 16}px"
        class="w-full text-left pr-3 py-[5px] cursor-pointer flex items-center gap-[7px] hover:bg-[#f1f3f7] {row.kind === 'diagram' && row.key === activeDiagramKey ? 'text-[#1e88e5] font-[600]' : 'text-slate-900'}"
      >
        <Icon size={15} class="flex-shrink-0 text-slate-500" />
        <span class="truncate flex-1">{row.title}</span>
        {#if row.kind === "diagram" && row.key === activeDiagramKey}
          <Check size={15} class="flex-shrink-0 text-[#1e88e5]" />
        {/if}
      </button>
    {/each}
  </div>
</div>
