<script lang="ts">
  // The navigator sheet — a prop-driven presentational tree grown from the
  // TopBar switcher. All mutations are callbacks so it unit-tests like TopBar.
  import { Check, ChevronDown, Folder, FileText, StickyNote, Box } from "lucide-svelte";
  import { buildNavTree, packageOf, type NavRow, type NavKind } from "@uaml/core/nav/tree";
  import { filterNav } from "@uaml/core/nav/search";
  import { GripVertical } from "lucide-svelte";
  import type { ModelGraph } from "@uaml/okf";

  let {
    graph,
    scopeKey = "",
    activeDiagramKey = "",
    palette = [],
    onScope,
    onSelectDiagram,
    onReorder,
    onViewInDiagram,
    onAddToNewDiagram,
    onEditProperties,
  }: {
    graph: ModelGraph;
    scopeKey?: string;
    activeDiagramKey?: string;
    palette?: string[];
    onScope?: (key: string) => void;
    onSelectDiagram?: (key: string) => void;
    onReorder?: (pkgKey: string, order: string[]) => void;
    onViewInDiagram?: (key: string, diagramKey: string) => void;
    onAddToNewDiagram?: (key: string) => void;
    onEditProperties?: (key: string) => void;
  } = $props();

  // Search box (filtering lands in Task 21; here it only toggles filterNav).
  let query = $state("");

  // Type chip: rotates through ["all", ...palette]; label de-prefixes the token.
  let typeFilter = $state("all");
  const chipOptions = $derived(["all", ...palette]);
  function rotateChip() {
    const i = chipOptions.indexOf(typeFilter);
    typeFilter = chipOptions[(i + 1) % chipOptions.length];
  }
  const deprefix = (t: string) => (t === "all" ? "All" : t.split(".").pop() || t);

  // Ctrl-T also rotates the chip (no inline key hint rendered).
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && (e.key === "t" || e.key === "T")) {
        e.preventDefault();
        rotateChip();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // The visible rows: filtered when searching, else the full scoped subtree.
  const rows = $derived<NavRow[]>(
    query ? filterNav(graph, scopeKey, query, typeFilter).inScope : buildNavTree(graph, scopeKey),
  );

  // Drag-reorder: track the dragged member; dropping on a same-package row
  // persists the reordered member keys via onReorder.
  let dragKey = $state<string | null>(null);
  function dropOn(target: NavRow) {
    const src = dragKey;
    dragKey = null;
    if (!src || src === target.key) return;
    const pkgKey = packageOf(graph, src);
    if (packageOf(graph, target.key) !== pkgKey) return; // only reorder within a package
    const members = (graph.packages.find((p) => p.key === pkgKey)?.members ?? []).filter((k) => k !== src);
    const at = members.indexOf(target.key);
    members.splice(at < 0 ? members.length : at, 0, src);
    onReorder?.(pkgKey, members);
  }

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

  // Left-click action menu for classifier/note rows (packages/diagrams keep
  // their scope/select behavior).
  let actionMenu = $state<{ key: string } | null>(null);
  const containing = $derived(
    actionMenu ? graph.diagrams.filter((d) => d.members.includes(actionMenu!.key)) : [],
  );

  function activateRow(row: NavRow) {
    if (row.kind === "package") onScope?.(row.key);
    else if (row.kind === "diagram") onSelectDiagram?.(row.key);
    else actionMenu = { key: row.key };
  }

  function closeMenus() {
    actionMenu = null;
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
      onclick={rotateChip}
      class="flex items-center gap-[3px] px-2 py-[6px] rounded-md border border-[#d8dee8] text-slate-600 cursor-pointer hover:bg-[#f1f3f7]"
    >
      {deprefix(typeFilter)} <ChevronDown size={13} class="text-slate-400" />
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
        aria-selected={row.kind === "diagram" && row.key === activeDiagramKey}
        draggable="true"
        ondragstart={() => (dragKey = row.key)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => dropOn(row)}
        onclick={() => activateRow(row)}
        style="padding-left:{8 + row.depth * 16}px"
        class="group w-full text-left pr-3 py-[5px] cursor-pointer flex items-center gap-[7px] hover:bg-[#f1f3f7] {row.kind === 'diagram' && row.key === activeDiagramKey ? 'text-[#1e88e5] font-[600]' : 'text-slate-900'}"
      >
        <GripVertical
          size={13}
          class="flex-shrink-0 text-slate-300 opacity-0 group-hover:opacity-100 cursor-grab"
          aria-hidden="true"
        />
        <Icon size={15} class="flex-shrink-0 text-slate-500" />
        <span class="truncate flex-1">{row.title}</span>
        {#if row.kind === "diagram" && row.key === activeDiagramKey}
          <Check size={15} class="flex-shrink-0 text-[#1e88e5]" />
        {/if}
      </button>
    {/each}
  </div>

  {#if actionMenu}
    {@const key = actionMenu.key}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={closeMenus}></div>
    <div
      role="menu"
      class="absolute z-50 left-1/2 -translate-x-1/2 top-[120px] w-[220px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
    >
      {#if containing.length <= 1}
        {#if containing.length === 1}
          <button
            role="menuitem"
            onclick={() => { onViewInDiagram?.(key, containing[0].key); closeMenus(); }}
            class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
          >
            View in diagram
          </button>
        {/if}
        <button
          role="menuitem"
          onclick={() => { onAddToNewDiagram?.(key); closeMenus(); }}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          Add to new diagram
        </button>
      {:else}
        <div class="px-3 py-2 text-[12px] text-slate-500">View in diagram</div>
        {#each containing as d (d.key)}
          <button
            role="menuitem"
            onclick={() => { onViewInDiagram?.(key, d.key); closeMenus(); }}
            class="w-full text-left text-[13px] text-slate-900 pl-6 pr-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
          >
            {d.title}
          </button>
        {/each}
        <button
          role="menuitem"
          onclick={() => { onAddToNewDiagram?.(key); closeMenus(); }}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          Add to new diagram
        </button>
      {/if}
      <div class="my-1 border-t border-[#eef1f5]"></div>
      <button
        role="menuitem"
        onclick={() => { onEditProperties?.(key); closeMenus(); }}
        class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
      >
        View / edit properties
      </button>
    </div>
  {/if}
</div>
