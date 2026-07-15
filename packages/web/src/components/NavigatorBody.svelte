<script lang="ts">
  // The navigator sheet — a prop-driven presentational tree grown from the
  // TopBar switcher. All mutations are callbacks so it unit-tests like TopBar.
  import { Check, ChevronDown, Folder, FileText, StickyNote, Box, Workflow, ArrowRightLeft } from "lucide-svelte";
  import { buildNavTree, packageOf, type NavRow, type NavKind } from "@waml/core/nav/tree";
  import { filterNav, matchSpan } from "@waml/core/nav/search";
  import { GripVertical } from "lucide-svelte";
  import type { ModelGraph } from "@waml/okf";

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
    onCreatePackage,
    onCreateNode,
    onCreateDiagram,
    onRename,
    onSort,
    onDelete,
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
    onCreatePackage?: (parentKey: string, name: string) => void;
    onCreateNode?: (dir: string, metaclass: string) => void;
    onCreateDiagram?: (name: string) => void;
    onRename?: (key: string, kind: NavKind, title: string) => void;
    onSort?: (pkgKey: string) => void;
    onDelete?: (key: string, kind: NavKind, mode: "single" | "cascade" | "reparent") => void;
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

  // The full scoped subtree (shown when not searching).
  const rows = $derived<NavRow[]>(buildNavTree(graph, scopeKey));
  // While searching, the three-state filtered result (matches / empty-scope /
  // empty-all); null clears the search overlay.
  const search = $derived(query ? filterNav(graph, scopeKey, query, typeFilter) : null);
  const scopeLabel = $derived(scopeKey.split("/").filter(Boolean).pop() || (graph.path || "model"));

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
    flow: Workflow,
    sequence: ArrowRightLeft,
  };

  // Flow/sequence rows are read-only behavior-doc views: no rename/delete/
  // reorder/properties affordances apply, so their "active" indicator only
  // needs to track activeDiagramKey (mirrors the diagram row's indicator).
  function isActiveRow(row: NavRow): boolean {
    return (row.kind === "diagram" || row.kind === "flow" || row.kind === "sequence") && row.key === activeDiagramKey;
  }

  // Left-click action menu for classifier/note rows (packages/diagrams keep
  // their scope/select behavior).
  let actionMenu = $state<{ key: string } | null>(null);
  const containing = $derived(
    actionMenu ? graph.diagrams.filter((d) => d.members.includes(actionMenu!.key)) : [],
  );

  function activateRow(row: NavRow) {
    if (row.kind === "package") onScope?.(row.key);
    else if (row.kind === "diagram" || row.kind === "flow" || row.kind === "sequence") onSelectDiagram?.(row.key);
    else actionMenu = { key: row.key };
  }

  // A row clicked from search results: rescoping to a package also clears the
  // query (the sheet returns to the plain scoped tree); other kinds behave as
  // in the main tree.
  function activateResult(row: NavRow) {
    if (row.kind === "package") {
      onScope?.(row.key);
      query = "";
    } else activateRow(row);
  }

  // Right-click context menu. `mode` reveals an inline input for the create /
  // rename actions (never window.prompt), mirroring TopBar's newMode pattern.
  let ctxMenu = $state<{ key: string; kind: NavKind; title: string } | null>(null);
  let ctxMode = $state<null | "package" | "diagram" | "rename">(null);
  let ctxInput = $state("");
  const ctxTargetPkg = $derived(
    ctxMenu ? (ctxMenu.kind === "package" ? ctxMenu.key : packageOf(graph, ctxMenu.key)) : "",
  );

  function openCtx(row: NavRow) {
    // Flow/sequence rows are read-only behavior-doc views: rename/delete/
    // reorder/sort/properties are not meaningful for them, so skip opening
    // the context menu entirely rather than wiring dead menu items.
    if (row.kind === "flow" || row.kind === "sequence") return;
    actionMenu = null;
    ctxMode = null;
    ctxInput = "";
    ctxMenu = { key: row.key, kind: row.kind, title: row.title };
  }

  // Delete prompt for a non-empty package (cascade / reparent / cancel). Empty
  // packages and non-package rows delete straight through onDelete.
  let deletePrompt = $state<{ key: string; title: string } | null>(null);
  function requestDelete() {
    if (!ctxMenu) return;
    const { key, kind, title } = ctxMenu;
    const members = graph.packages.find((p) => p.key === key)?.members ?? [];
    if (kind === "package" && members.length > 0) {
      ctxMenu = null;
      ctxMode = null;
      deletePrompt = { key, title };
    } else {
      onDelete?.(key, kind, "single");
      closeMenus();
    }
  }

  function closeMenus() {
    actionMenu = null;
    ctxMenu = null;
    ctxMode = null;
    deletePrompt = null;
  }

  function startMode(mode: "package" | "diagram" | "rename") {
    ctxMode = mode;
    ctxInput = mode === "rename" ? (ctxMenu?.title ?? "") : "";
  }

  function submitCtx() {
    const name = ctxInput.trim();
    if (!name || !ctxMenu) return;
    if (ctxMode === "package") onCreatePackage?.(ctxTargetPkg, name);
    else if (ctxMode === "diagram") onCreateDiagram?.(name);
    else if (ctxMode === "rename") onRename?.(ctxMenu.key, ctxMenu.kind, name);
    closeMenus();
  }
</script>

<div role="menu" tabindex="-1" class="relative flex flex-col h-full min-h-0 py-1 text-[13px] text-slate-900">
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

  <!-- Title with the matched substring wrapped in <mark> (search results). -->
  {#snippet marked(title: string)}
    {@const span = matchSpan(title, query)}
    {#if span}{title.slice(0, span[0])}<mark class="rounded-[2px] bg-[#fff3bf] text-inherit"
        >{title.slice(span[0], span[1])}</mark
      >{title.slice(span[1])}{:else}{title}{/if}
  {/snippet}

  <!-- A single search-result row (no drag; highlighted title). -->
  {#snippet resultRow(row: NavRow)}
    {@const Icon = KIND_ICON[row.kind]}
    <button
      role="treeitem"
      aria-label={row.title}
      aria-selected={isActiveRow(row)}
      oncontextmenu={(e) => { e.preventDefault(); openCtx(row); }}
      onclick={() => activateResult(row)}
      style="padding-left:{8 + row.depth * 16}px"
      class="w-full text-left pr-3 py-[5px] cursor-pointer flex items-center gap-[7px] text-slate-900 hover:bg-[#f1f3f7]"
    >
      <Icon size={15} class="flex-shrink-0 text-slate-500" />
      <span class="truncate flex-1">{@render marked(row.title)}</span>
    </button>
  {/snippet}

  <!-- Tree -->
  <div class="flex-1 min-h-0 overflow-y-auto py-0.5">
    {#if search}
      {#if search.state === "empty-all"}
        <div class="px-3 py-6 text-center text-[12.5px] text-slate-400">No matches found</div>
      {:else if search.state === "empty-scope"}
        <div class="px-3 py-3 text-center text-[12.5px] text-slate-400">No matches in {scopeLabel}</div>
        <div class="px-3 pb-1 text-[11px] font-[600] uppercase tracking-wide text-slate-400">Elsewhere in model</div>
        {#each search.elsewhere as row (row.key)}
          {@render resultRow(row)}
        {/each}
      {:else}
        {#each search.inScope as row (row.key)}
          {@render resultRow(row)}
        {/each}
      {/if}
    {:else}
      {#each rows as row (row.key)}
        {@const Icon = KIND_ICON[row.kind]}
        <button
          role="treeitem"
          aria-selected={isActiveRow(row)}
          draggable="true"
          ondragstart={() => (dragKey = row.key)}
          ondragover={(e) => e.preventDefault()}
          ondrop={() => dropOn(row)}
          oncontextmenu={(e) => { e.preventDefault(); openCtx(row); }}
          onclick={() => activateRow(row)}
          style="padding-left:{8 + row.depth * 16}px"
          class="group w-full text-left pr-3 py-[5px] cursor-pointer flex items-center gap-[7px] hover:bg-[#f1f3f7] {isActiveRow(row) ? 'text-[#1e88e5] font-[600]' : 'text-slate-900'}"
        >
          <GripVertical
            size={13}
            class="flex-shrink-0 text-slate-300 opacity-0 group-hover:opacity-100 cursor-grab"
            aria-hidden="true"
          />
          <Icon size={15} class="flex-shrink-0 text-slate-500" />
          <span class="truncate flex-1">{row.title}</span>
          {#if isActiveRow(row)}
            <Check size={15} class="flex-shrink-0 text-[#1e88e5]" />
          {/if}
        </button>
      {/each}
    {/if}
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

  {#if ctxMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={closeMenus}></div>
    <div
      role="menu"
      class="absolute z-50 left-1/2 -translate-x-1/2 top-[120px] w-[230px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
    >
      <!-- New package — inline name input -->
      {#if ctxMode === "package"}
        <form class="px-2 py-1 flex items-center gap-1.5" onsubmit={(e) => { e.preventDefault(); submitCtx(); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            aria-label="New package name"
            bind:value={ctxInput}
            placeholder="New package name"
            autofocus
            class="flex-1 min-w-0 text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
          />
          <button type="submit" class="text-[12.5px] font-[550] text-[#1e88e5] px-2 py-[6px] rounded-md cursor-pointer hover:bg-[#e6f1fb]">Add</button>
        </form>
      {:else}
        <button
          role="menuitem"
          onclick={() => startMode("package")}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          New package
        </button>
      {/if}

      <!-- One create item per palette metaclass, de-prefixed for the label -->
      {#each palette as token (token)}
        <button
          role="menuitem"
          onclick={() => { onCreateNode?.(ctxTargetPkg, token); closeMenus(); }}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          New {deprefix(token)}
        </button>
      {/each}

      <!-- New diagram — inline name input -->
      {#if ctxMode === "diagram"}
        <form class="px-2 py-1 flex items-center gap-1.5" onsubmit={(e) => { e.preventDefault(); submitCtx(); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            aria-label="New diagram name"
            bind:value={ctxInput}
            placeholder="New diagram name"
            autofocus
            class="flex-1 min-w-0 text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
          />
          <button type="submit" class="text-[12.5px] font-[550] text-[#1e88e5] px-2 py-[6px] rounded-md cursor-pointer hover:bg-[#e6f1fb]">Add</button>
        </form>
      {:else}
        <button
          role="menuitem"
          onclick={() => startMode("diagram")}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          New diagram
        </button>
      {/if}

      <div class="my-1 border-t border-[#eef1f5]"></div>

      <!-- Rename — inline input seeded with the current title -->
      {#if ctxMode === "rename"}
        <form class="px-2 py-1 flex items-center gap-1.5" onsubmit={(e) => { e.preventDefault(); submitCtx(); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            aria-label="Rename item"
            bind:value={ctxInput}
            placeholder="Rename"
            autofocus
            class="flex-1 min-w-0 text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
          />
          <button type="submit" class="text-[12.5px] font-[550] text-slate-600 px-2 py-[6px] rounded-md cursor-pointer hover:bg-[#f1f3f7]">Rename</button>
        </form>
      {:else}
        <button
          role="menuitem"
          onclick={() => startMode("rename")}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
        >
          Rename
        </button>
      {/if}

      <button
        role="menuitem"
        onclick={() => { onSort?.(ctxTargetPkg); closeMenus(); }}
        class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
      >
        Sort A–Z
      </button>

      <div class="my-1 border-t border-[#eef1f5]"></div>

      <button
        role="menuitem"
        onclick={requestDelete}
        class="w-full text-left text-[13px] text-[#d64545] px-3 py-2 cursor-pointer hover:bg-[#fdeded]"
      >
        Delete…
      </button>
    </div>
  {/if}

  {#if deletePrompt}
    {@const key = deletePrompt.key}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={closeMenus}></div>
    <div
      role="menu"
      class="absolute z-50 left-1/2 -translate-x-1/2 top-[120px] w-[230px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
    >
      <div class="px-3 py-2 text-[12.5px] text-slate-600">
        Delete <span class="font-[600] text-slate-900">{deletePrompt.title}</span>?
      </div>
      <button
        onclick={() => { onDelete?.(key, "package", "cascade"); closeMenus(); }}
        class="w-full text-left text-[13px] text-[#d64545] px-3 py-2 cursor-pointer hover:bg-[#fdeded]"
      >
        Delete children too
      </button>
      <button
        onclick={() => { onDelete?.(key, "package", "reparent"); closeMenus(); }}
        class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
      >
        Move to parent
      </button>
      <button
        onclick={closeMenus}
        class="w-full text-left text-[13px] text-slate-500 px-3 py-2 cursor-pointer hover:bg-[#f1f3f7]"
      >
        Cancel
      </button>
    </div>
  {/if}
</div>
