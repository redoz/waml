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
  <!-- Brand — logo links to owox.com -->
  <div class="flex items-center gap-[9px] font-[650] text-[15px] tracking-[-0.2px]">
    <a
      href="https://owox.com"
      target="_blank"
      rel="noreferrer"
      title="OWOX — owox.com"
      aria-label="OWOX — owox.com"
      class="flex items-center rounded-md transition-opacity hover:opacity-80"
    >
      <svg viewBox="0 0 512 512" fill="none" xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M421.311 119.85C435.258 133.807 440.996 157.327 440.996 157.327C440.996 157.327 449.53 204.69 449.53 268.995C449.53 177.972 418.65 162.348 311.314 162.348H212.327C157.38 162.348 161.097 217.57 157.38 243.85L152.865 283.556C150.697 325.33 157.951 351.215 200.811 351.215C111.444 351.215 61.806 365.847 61.8062 239.866C61.8061 182.846 70.4043 157.327 70.4043 157.327C70.4043 157.327 76.1419 133.807 90.1183 119.85C104.095 105.877 124.809 104.475 124.809 104.475C124.809 104.475 167.579 98.0374 252.066 98.0374C336.554 98.0374 384.285 104.475 384.285 104.475C384.285 104.475 407.321 105.877 421.311 119.85Z" fill="url(#topbar-g0)"/>
        <path d="M449.515 271.888C449.52 273.026 449.523 274.174 449.523 275.333C449.523 329.946 441.393 351.201 441.393 351.201C441.393 351.201 435.03 376.952 424.167 388.075C406.929 405.725 388.495 406.71 388.495 406.71C388.495 406.71 348.836 413.061 263.502 413.061C181.632 413.061 127.111 406.749 127.111 406.749C127.111 406.749 104.091 405.337 90.1144 391.377C76.1379 377.394 70.4004 351.201 70.4004 351.201C70.4004 351.201 61.8062 297.401 61.8062 238.506C61.806 352.055 102.131 351.374 175.525 350.133C183.56 349.998 191.992 349.855 200.811 349.855H299.787C343.122 349.855 352.906 318.315 354.792 282.196L359.32 227.093C360.526 204.443 357.608 188.362 350.507 178.012C342.765 166.722 329.575 160.987 311.314 160.987C424.974 160.987 448.73 176.216 449.515 271.888Z" fill="url(#topbar-g1)"/>
        <defs>
          <linearGradient id="topbar-g0" x1="255.15" y1="98" x2="256.871" y2="367" gradientUnits="userSpaceOnUse">
            <stop stop-color="#05D2FF"/>
            <stop offset=".15" stop-color="#21A1F1"/>
            <stop offset=".4" stop-color="#1E88E5"/>
            <stop offset=".72" stop-color="#1E6EE5"/>
            <stop offset="1" stop-color="#182FFF"/>
          </linearGradient>
          <linearGradient id="topbar-g1" x1="85.6" y1="412.6" x2="394" y2="143.8" gradientUnits="userSpaceOnUse">
            <stop stop-color="#24D8FF"/>
            <stop offset=".15" stop-color="#21A1F1"/>
            <stop offset=".4" stop-color="#1E88E5"/>
            <stop offset=".75" stop-color="#1E7AE5"/>
            <stop offset="1" stop-color="#0046F9"/>
          </linearGradient>
        </defs>
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
