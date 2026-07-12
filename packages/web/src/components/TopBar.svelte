<script lang="ts">
  // Mirrors packages/web/src/components/TopBar.tsx.
  import { Download, Upload, ChevronDown, Check, Plus, FileText, Image as ImageIcon, Share2 } from "lucide-svelte";
  import { LibraryIcon } from "../lib/icons";
  import type { Diagram } from "@uaml/okf";

  // First-visit onboarding hint pointing at the Library. Persisted so it only
  // ever shows once per browser; dismissed as soon as the user hovers it.
  const LIBRARY_HINT_KEY = "mc.libraryHint.v1";

  // Share is now a first-class top-bar button (immediately right of Export) that
  // opens the modal Share dialog — it no longer lives in the right rail.
  let {
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
    onRenameDiagram,
    onCreateDiagram,
  }: {
    onImport?: () => void;
    onExport?: () => void;
    onExportSvg?: () => void;
    exportDisabled?: boolean;
    onShare?: () => void;
    shareDisabled?: boolean;
    onLibrary?: () => void;
    // Diagram title switcher — replaces the old Business Goal button and the
    // floating DiagramTabs pill. The active diagram's title doubles as the
    // dropdown trigger (switch / rename-current / create-new).
    diagrams?: Diagram[];
    activeDiagramKey?: string;
    onSelectDiagram?: (key: string) => void;
    onRenameDiagram?: (title: string) => void;
    onCreateDiagram?: (name: string) => void;
  } = $props();

  // Export dropdown (OKF markdown / SVG).
  let exportMenuOpen = $state(false);
  // Show the Library hint on first ever visit; stays lit until hovered.
  let showLibraryHint = $state(false);

  // ── Diagram title switcher ─────────────────────────────────────────────────
  let switcherOpen = $state(false);
  // Inline rename field, seeded from the active title each time the menu opens.
  let renameValue = $state("");
  // "+ New diagram" reveals an inline name input (never window.prompt).
  let newMode = $state(false);
  let newName = $state("");

  const activeTitle = $derived(
    diagrams.find((d) => d.key === activeDiagramKey)?.title ?? diagrams[0]?.title ?? "Untitled diagram",
  );

  function openSwitcher() {
    switcherOpen = !switcherOpen;
    if (switcherOpen) {
      renameValue = activeTitle;
      newMode = false;
      newName = "";
    }
  }

  function selectDiagram(key: string) {
    onSelectDiagram?.(key);
    switcherOpen = false;
  }

  function submitRename() {
    const title = renameValue.trim();
    if (!title) return; // reject empty/whitespace — keeps the previous title
    onRenameDiagram?.(title);
    switcherOpen = false;
  }

  function submitNew() {
    const name = newName.trim();
    if (!name) return;
    onCreateDiagram?.(name);
    newName = "";
    newMode = false;
    switcherOpen = false;
  }

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

<div class="flex items-center gap-3 px-4 py-[9px] bg-white border-b border-[#d8dee8] flex-shrink-0 z-30">
  <!-- Brand — UAML wordmark links to the GitHub repo -->
  <div class="flex items-center gap-[9px] font-[650] text-[15px] tracking-[-0.2px]">
    <a
      href="https://github.com/redoz/uaml"
      target="_blank"
      rel="noreferrer"
      title="UAML — github.com/redoz/uaml"
      aria-label="UAML — github.com/redoz/uaml"
      class="flex items-center rounded-md transition-opacity hover:opacity-80"
    >
      <!-- UAML wordmark. Inlined (matching the previous pattern) and filled with
           currentColor so it inherits the brand text color and dims on hover. -->
      <svg
        viewBox="-20 -20 440 140"
        xmlns="http://www.w3.org/2000/svg"
        width="75"
        height="24"
        role="img"
        aria-label="UAML"
      >
        <g fill="currentColor">
          <!-- U -->
          <path d="M 0,0 H 25 V 75 H 55 V 0 H 80 V 85 L 65,100 H 15 L 0,85 Z" transform="translate(0, 0)" />
          <!-- A -->
          <path fill-rule="evenodd" d="M 0,100 V 15 L 15,0 H 65 L 80,15 V 100 H 55 V 65 H 25 V 100 Z M 25,25 H 55 V 40 H 25 Z" transform="translate(100, 0)" />
          <!-- M -->
          <path d="M 0,100 V 0 H 25 L 50,40 L 75,0 H 100 V 100 H 75 V 45 L 50,75 L 25,45 V 100 Z" transform="translate(200, 0)" />
          <!-- L -->
          <path d="M 0,0 H 25 V 75 H 80 V 85 L 65,100 H 15 L 0,85 Z" transform="translate(320, 0)" />
        </g>
      </svg>
    </a>
    <span>Model Canvas</span>
  </div>

  <div class="flex-1"></div>

  <!-- Diagram title & switcher — centered. The active diagram's title doubles as
       the switcher trigger; the dropdown switches diagram, renames the current
       one, or creates a new (empty) diagram. Keeps the blue treatment carried
       over from the old Business Goal button (Target icon dropped). -->
  <div class="relative">
    <button
      onclick={openSwitcher}
      aria-label={`Diagram: ${activeTitle} — switch diagram`}
      aria-haspopup="menu"
      aria-expanded={switcherOpen}
      title="Switch, rename, or create a diagram"
      class="flex items-center gap-[6px] rounded-lg px-[10px] py-[6px] text-[13px] font-[600] cursor-pointer transition-colors text-[#1e88e5] bg-[#e6f1fb] hover:bg-[#d8e8f9]"
    >
      <span class="max-w-[240px] truncate">{activeTitle}</span>
      <ChevronDown size={14} class="text-[#1e88e5]/70" />
    </button>
    {#if switcherOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (switcherOpen = false)}></div>
      <div
        role="menu"
        class="absolute top-[calc(100%+6px)] left-1/2 -translate-x-1/2 z-50 w-[248px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
      >
        <!-- Diagram list — switch on click; the active one is checkmarked. -->
        {#each diagrams as d (d.key)}
          <button
            role="menuitemradio"
            aria-checked={d.key === activeDiagramKey}
            onclick={() => selectDiagram(d.key)}
            class="w-full text-left text-[13px] px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[#f1f3f7] {d.key === activeDiagramKey ? 'text-[#1e88e5] font-[600]' : 'text-slate-900'}"
          >
            <span class="w-[15px] flex-shrink-0">
              {#if d.key === activeDiagramKey}<Check size={15} class="text-[#1e88e5]" />{/if}
            </span>
            <span class="truncate">{d.title}</span>
          </button>
        {/each}

        <div class="my-1 border-t border-[#eef1f5]"></div>

        <!-- Rename the current diagram — inline; empty/whitespace is rejected. -->
        <form class="px-2 py-1 flex items-center gap-1.5" onsubmit={(e) => { e.preventDefault(); submitRename(); }}>
          <input
            aria-label="Rename diagram"
            bind:value={renameValue}
            placeholder="Rename diagram"
            class="flex-1 min-w-0 text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
          />
          <button
            type="submit"
            class="text-[12.5px] font-[550] text-slate-600 px-2 py-[6px] rounded-md cursor-pointer hover:bg-[#f1f3f7]"
          >
            Rename
          </button>
        </form>

        <div class="my-1 border-t border-[#eef1f5]"></div>

        <!-- Create a new (empty) diagram — inline name input, not window.prompt. -->
        {#if newMode}
          <form class="px-2 py-1 flex items-center gap-1.5" onsubmit={(e) => { e.preventDefault(); submitNew(); }}>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              aria-label="New diagram name"
              bind:value={newName}
              placeholder="New diagram name"
              autofocus
              class="flex-1 min-w-0 text-[13px] px-2 py-[6px] border border-[#d8dee8] rounded-md text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"
            />
            <button
              type="submit"
              class="text-[12.5px] font-[550] text-[#1e88e5] px-2 py-[6px] rounded-md cursor-pointer hover:bg-[#e6f1fb]"
            >
              Create
            </button>
          </form>
        {:else}
          <button
            onclick={() => (newMode = true)}
            class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[#f1f3f7]"
          >
            <Plus size={15} class="text-slate-500" /> New diagram
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex-1"></div>

  <!-- Templates -->
  <div class="relative">
    <!-- Pulsing ring highlights the Templates control on first visit -->
    {#if showLibraryHint}
      <span class="absolute -inset-[3px] rounded-[10px] ring-2 ring-[#1e88e5]/60 animate-pulse pointer-events-none"></span>
    {/if}
    <button
      onclick={() => {
        dismissLibraryHint();
        onLibrary?.();
      }}
      title="Browse model templates"
      class="text-[13px] font-[550] text-slate-900 border border-[#d8dee8] bg-white rounded-lg px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[#f1f3f7]"
    >
      <LibraryIcon size={15} /> Templates
    </button>
    {#if showLibraryHint}
      <div
        role="tooltip"
        onmouseenter={dismissLibraryHint}
        class="absolute top-[calc(100%+11px)] right-0 z-40 w-[232px] rounded-lg bg-slate-900 text-white text-[12.5px] leading-[1.45] px-3 py-2.5 shadow-[0_8px_24px_rgba(15,23,42,0.28)] cursor-default"
      >
        <span class="absolute -top-[5px] right-[18px] w-[10px] h-[10px] bg-slate-900 rotate-45"></span>
        Roll out a ready-made model from the templates — or build your own from scratch.
      </div>
    {/if}
  </div>

  <!-- Import OKF -->
  <button
    onclick={onImport}
    class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[#f1f3f7]"
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
      class="text-[13px] font-[550] border border-[#d8dee8] bg-white text-slate-900 rounded-lg px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[#f1f3f7] disabled:opacity-50 disabled:cursor-not-allowed"
    >
      <Upload size={15} /> Export <ChevronDown size={14} class="text-slate-400" />
    </button>
    {#if exportMenuOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (exportMenuOpen = false)}></div>
      <div role="menu" class="absolute top-[calc(100%+6px)] right-0 z-50 w-[232px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1">
        <button
          role="menuitem"
          onclick={() => {
            exportMenuOpen = false;
            onExport?.();
          }}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[#f1f3f7]"
        >
          <FileText size={15} class="text-slate-500" /> OKF (Markdown)
        </button>
        <button
          role="menuitem"
          onclick={() => {
            exportMenuOpen = false;
            onExportSvg?.();
          }}
          class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[#f1f3f7]"
        >
          <ImageIcon size={15} class="text-slate-500" /> Image (SVG)
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
    class="text-[13px] font-[550] bg-[#1e88e5] text-white rounded-lg px-3 py-[7px] cursor-pointer flex items-center gap-[6px] hover:bg-[#1976d2] disabled:opacity-50 disabled:cursor-not-allowed"
  >
    <Share2 size={15} /> Share
  </button>
</div>
