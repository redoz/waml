<script lang="ts">
  // Mirrors packages/web/src/components/TopBar.tsx.
  import { Download, Upload, ChevronDown, Target, FileText, Image as ImageIcon } from "lucide-svelte";
  import { LibraryIcon } from "../lib/icons";

  // First-visit onboarding hint pointing at the Library. Persisted so it only
  // ever shows once per browser; dismissed as soon as the user hovers it.
  const LIBRARY_HINT_KEY = "mc.libraryHint.v1";

  // onShare / shareDisabled are part of the prop contract (parity with
  // TopBarProps) but, exactly as in TopBar.tsx, are not destructured or
  // rendered here — Share lives in the right rail.
  let {
    onImport,
    onExport,
    onExportSvg,
    exportDisabled = false,
    onLibrary,
    onOpenGoal,
    goalSet = false,
  }: {
    onImport?: () => void;
    onExport?: () => void;
    onExportSvg?: () => void;
    exportDisabled?: boolean;
    onShare?: () => void;
    shareDisabled?: boolean;
    onLibrary?: () => void;
    onOpenGoal?: () => void;
    goalSet?: boolean;
  } = $props();

  // Export dropdown (OKF markdown / SVG).
  let exportMenuOpen = $state(false);
  // Show the Library hint on first ever visit; stays lit until hovered.
  let showLibraryHint = $state(false);

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

  <!-- Business Goal — capture the objective behind the model. Stored locally
       alongside the model; a standalone entry point (no server dependency). -->
  <button
    onclick={onOpenGoal}
    aria-label="Business goal"
    title="Set the business goal behind this model"
    class={`flex items-center gap-[6px] rounded-lg px-[10px] py-[6px] text-[13px] font-[550] cursor-pointer transition-colors ${goalSet ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900"}`}
  >
    <Target size={16} /> {goalSet ? "Business goal" : "Set business goal"}
  </button>

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
      title={exportDisabled ? "Add a mart first, then export" : "Export this model"}
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

  <!-- Share lives in the right rail now — no top-bar buttons. -->
</div>
