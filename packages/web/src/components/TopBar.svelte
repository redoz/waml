<script lang="ts">
  // Mirrors packages/web/src/components/TopBar.tsx.
  import { Download, Upload, ChevronDown, FileText, Image as ImageIcon, Share2, PanelLeft, Pencil, Check, Plus } from "lucide-svelte";
  import { LibraryIcon } from "../lib/icons";
  import type { Diagram } from "@waml/okf";

  // First-visit onboarding hint pointing at the Library. Persisted so it only
  // ever shows once per browser; dismissed as soon as the user hovers it.
  const LIBRARY_HINT_KEY = "mc.libraryHint.v1";

  // Share is now a first-class top-bar button (immediately right of Export) that
  // opens the modal Share dialog — it no longer lives in the right rail.
  let {
    onCreateNew,
    onImport,
    onExport,
    onExportSvg,
    exportDisabled = false,
    onShare,
    shareDisabled = false,
    onLibrary,
    diagrams = [],
    activeDiagramKey = "",
    onSelectDiagram,
    onDockModel,
    onEditModel,
    rootPackageName = "",
    onRenameRoot,
  }: {
    onCreateNew?: () => void;
    onImport?: () => void;
    onExport?: () => void;
    onExportSvg?: () => void;
    exportDisabled?: boolean;
    onShare?: () => void;
    shareDisabled?: boolean;
    onLibrary?: () => void;
    // Diagram title & switcher — centered. The active diagram's title doubles as
    // the switcher trigger; opens the read-only diagram switcher popover.
    diagrams?: Diagram[];
    activeDiagramKey?: string;
    onSelectDiagram?: (key: string) => void;
    onDockModel?: () => void;
    onEditModel?: () => void;
    // Name of the model's root package — shown as the brand subtitle.
    rootPackageName?: string;
    // Commit a new root package title (inline rename from the brand).
    onRenameRoot?: (title: string) => void;
  } = $props();

  // Export dropdown (OKF markdown / SVG).
  let exportMenuOpen = $state(false);
  // Show the Library hint on first ever visit; stays lit until hovered.
  let showLibraryHint = $state(false);
  // Read-only diagram switcher popover — same anchoring pattern as the Export
  // menu below (full-screen click-catcher + absolutely positioned card).
  let switcherOpen = $state(false);

  // ── Inline root-package rename ─────────────────────────────────────────────
  // Clicking the name (or the hover pencil) swaps it for a text input seeded with
  // the current name; Enter/blur commit a non-blank change, Esc cancels.
  let renaming = $state(false);
  let renameDraft = $state("");

  function startRename() {
    renameDraft = rootPackageName;
    renaming = true;
  }
  function commitRename() {
    if (!renaming) return;
    renaming = false;
    const next = renameDraft.trim();
    if (next && next !== rootPackageName) onRenameRoot?.(next);
  }
  function cancelRename() {
    renaming = false;
  }
  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelRename();
    }
  }

  // ── Diagram title switcher ─────────────────────────────────────────────────
  // Read-only diagram switcher popover — lists diagrams, checks the active one,
  // row-click selects. Dock/Edit buttons in header escalate to full editor/panel.
  // No diagram rename or create.
  const activeTitle = $derived(
    diagrams.find((d) => d.key === activeDiagramKey)?.title ?? diagrams[0]?.title ?? "Untitled diagram",
  );

  $effect(() => {
    if (!switcherOpen) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") switcherOpen = false;
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  $effect(() => {
    try {
      if (!localStorage.getItem(LIBRARY_HINT_KEY)) showLibraryHint = true;
    } catch {
      /* private mode */
    }
  });

  function dismissLibraryHint() {
    showLibraryHint = false;
    try {
      localStorage.setItem(LIBRARY_HINT_KEY, "seen");
    } catch {
      /* private mode */
    }
  }
</script>

<div class="relative flex items-center gap-3 px-4 py-[9px] bg-white border-b border-[color:var(--hair)] flex-shrink-0 z-30">
  <!-- Brand — WAML wordmark links to the GitHub repo; the root package name
       trails it as a subtitle. -->
  <div class="flex items-center gap-[9px] font-[700] text-[15px] tracking-[-0.2px]">
    <a
      href="https://github.com/redoz/waml"
      target="_blank"
      rel="noreferrer"
      title="WAML — github.com/redoz/waml"
      aria-label="WAML — github.com/redoz/waml"
      class="flex items-center rounded-[var(--round-chip)] transition-opacity hover:opacity-80"
    >
      WAML
    </a>
    <span class="text-[color:rgba(var(--ink-faint),.6)] font-normal">/</span>
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        aria-label="Package name"
        class="font-[600] text-[color:var(--ink)] max-w-[240px] px-1 py-0.5 rounded-[var(--round-chip)] border border-[color:var(--hair)] outline-none focus:border-[color:rgb(var(--accent))]"
        value={renameDraft}
        autofocus
        oninput={(e) => (renameDraft = e.currentTarget.value)}
        onkeydown={onRenameKey}
        onblur={commitRename}
      />
    {:else}
      <div class="group flex items-center gap-1">
        <!-- The name is a plain clickable span (NOT a button) so the pencil is the
             one and only "Rename package" button — keeps the test query
             deterministic. Both open the same inline editor. -->
        <span
          role="button"
          tabindex="0"
          onclick={startRename}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); startRename(); } }}
          title="Rename package"
          class="font-[600] text-[color:var(--ink-dim)] max-w-[240px] truncate cursor-text hover:text-[color:var(--ink)]"
        >
          {#if rootPackageName}{rootPackageName}{:else}<span class="text-[color:rgb(var(--ink-faint))] italic">Untitled</span>{/if}
        </span>
        <button
          type="button"
          onclick={startRename}
          title="Rename package"
          aria-label="Rename package"
          class="opacity-0 group-hover:opacity-100 transition-opacity text-[color:rgb(var(--ink-faint))] hover:text-[color:var(--ink-dim)]"
        >
          <Pencil size={13} />
        </button>
      </div>
    {/if}
  </div>

  <!-- Diagram title & switcher — centered. The active diagram's title doubles as
       the switcher trigger; the dropdown lists diagrams, checks the active one,
       and row-click selects. Dock/Edit buttons escalate to the full editor.
       No write actions. Keeps the blue treatment carried over from the old
       Business Goal button (Target icon dropped). Absolutely centered on the
       bar so it tracks the page center, not the gap between brand and buttons. -->
  <div class="absolute left-1/2 -translate-x-1/2">
    <button
      onclick={() => (switcherOpen = !switcherOpen)}
      aria-label={`Diagram: ${activeTitle} — switch diagram`}
      aria-haspopup="dialog"
      aria-expanded={switcherOpen}
      title="Switch diagram"
      class="flex items-center gap-[6px] rounded-[var(--round-chip)] px-[10px] py-[6px] text-[13px] font-[600] cursor-pointer transition-colors text-[color:rgb(var(--accent))] bg-[color:rgba(var(--accent),.12)] hover:bg-[color:rgba(var(--accent),.20)]"
    >
      <span class="max-w-[240px] truncate">{activeTitle}</span>
      <ChevronDown size={14} class="text-[color:rgba(var(--accent),.7)]" />
    </button>
    {#if switcherOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (switcherOpen = false)}></div>
      <div
        role="dialog"
        aria-label="Switch diagram"
        class="absolute top-[calc(100%+6px)] left-1/2 -translate-x-1/2 z-50 w-[300px] rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-white shadow-[0_12px_30px_rgba(40,70,110,.20)]"
      >
        <div class="flex items-center gap-1 px-2 py-1.5 border-b border-[color:var(--hair)]">
          <span class="flex-1 text-[12px] font-[600] text-[color:rgb(var(--ink-faint))] px-1">Diagrams</span>
          <button
            onclick={() => {
              switcherOpen = false;
              onDockModel?.();
            }}
            aria-label="Dock model editor"
            title="Dock the model editor to the left"
            class="w-[28px] h-[28px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)]"
          >
            <PanelLeft size={15} />
          </button>
          <button
            onclick={() => {
              switcherOpen = false;
              onEditModel?.();
            }}
            aria-label="Edit model"
            title="Open the full model editor"
            class="w-[28px] h-[28px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)]"
          >
            <Pencil size={15} />
          </button>
        </div>
        <div role="listbox" aria-label="Diagrams" class="py-1 max-h-[320px] overflow-y-auto">
          {#each diagrams as d (d.key)}
            <button
              role="option"
              aria-selected={d.key === activeDiagramKey}
              onclick={() => {
                switcherOpen = false;
                onSelectDiagram?.(d.key);
              }}
              class="w-full text-left text-[13px] text-[color:var(--ink)] px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[color:rgba(var(--accent),.12)]"
            >
              <FileText size={15} class="text-[color:rgb(var(--ink-faint))] flex-none" />
              <span class="flex-1 truncate">{d.title}</span>
              {#if d.key === activeDiagramKey}<Check size={15} class="text-[color:rgb(var(--accent))] flex-none" />{/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="flex-1"></div>

  <!-- Create new — resets to an empty project (confirm-gated in CanvasInner). -->
  <button
    onclick={onCreateNew}
    title="Create a new project"
    class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[color:rgba(var(--accent),.10)]"
  >
    <Plus size={15} /> Create new
  </button>

  <!-- Templates -->
  <div class="relative">
    <!-- Pulsing ring highlights the Templates control on first visit -->
    {#if showLibraryHint}
      <span class="absolute -inset-[3px] rounded-[var(--round-chip)] ring-2 ring-[rgba(var(--accent),.6)] animate-pulse pointer-events-none"></span>
    {/if}
    <button
      onclick={() => {
        dismissLibraryHint();
        onLibrary?.();
      }}
      title="Browse model templates"
      class="text-[13px] font-[600] text-[color:var(--ink)] border border-[color:var(--hair)] bg-white rounded-[var(--round-chip)] px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[color:rgba(var(--accent),.10)]"
    >
      <LibraryIcon size={15} /> Templates
    </button>
    {#if showLibraryHint}
      <div
        role="tooltip"
        onmouseenter={dismissLibraryHint}
        class="absolute top-[calc(100%+11px)] right-0 z-40 w-[232px] rounded-[var(--round-chip)] bg-[var(--ink)] text-white text-[12.5px] leading-[1.45] px-3 py-2.5 shadow-[0_12px_30px_rgba(40,70,110,.28)] cursor-default"
      >
        <span class="absolute -top-[5px] right-[18px] w-[10px] h-[10px] bg-[var(--ink)] rotate-45"></span>
        Roll out a ready-made model from the templates — or build your own from scratch.
      </div>
    {/if}
  </div>

  <!-- Import OKF -->
  <button
    onclick={onImport}
    class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[color:rgba(var(--accent),.10)]"
  >
    <Download size={15} /> Import
  </button>

  <!-- Export — dropdown: OKF markdown, SVG image -->
  <div class="relative">
    <button
      onclick={() => (exportMenuOpen = !exportMenuOpen)}
      disabled={exportDisabled}
      aria-haspopup="menu"
      aria-expanded={exportMenuOpen}
      title={exportDisabled ? "Add a node first, then export" : "Export this model"}
      class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[color:rgba(var(--accent),.10)] disabled:opacity-50 disabled:cursor-not-allowed"
    >
      <Upload size={15} /> Export <ChevronDown size={14} class="text-[color:rgb(var(--ink-faint))]" />
    </button>
    {#if exportMenuOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (exportMenuOpen = false)}></div>
      <div role="menu" class="absolute top-[calc(100%+6px)] right-0 z-50 w-[232px] rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-white shadow-[0_12px_30px_rgba(40,70,110,.20)] py-1">
        <button
          role="menuitem"
          onclick={() => {
            exportMenuOpen = false;
            onExport?.();
          }}
          class="w-full text-left text-[13px] text-[color:var(--ink)] px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[color:rgba(var(--accent),.12)]"
        >
          <FileText size={15} class="text-[color:rgb(var(--ink-faint))]" /> OKF (Markdown)
        </button>
        <button
          role="menuitem"
          onclick={() => {
            exportMenuOpen = false;
            onExportSvg?.();
          }}
          class="w-full text-left text-[13px] text-[color:var(--ink)] px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[color:rgba(var(--accent),.12)]"
        >
          <ImageIcon size={15} class="text-[color:rgb(var(--ink-faint))]" /> Image (SVG)
        </button>
      </div>
    {/if}
  </div>

  <!-- Share — first-class button, immediately right of Export. Opens the modal
       Share dialog (link + share-as-image). -->
  <button
    onclick={onShare}
    disabled={shareDisabled}
    title={shareDisabled ? "Add something to share" : "Share this model"}
    class="text-[13px] font-[600] bg-[color:rgb(var(--accent))] text-white rounded-[var(--round-chip)] px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:brightness-95 disabled:opacity-50 disabled:cursor-not-allowed"
  >
    <Share2 size={15} /> Share
  </button>
</div>
