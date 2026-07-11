<script module lang="ts">
  // Mirrors packages/web/src/components/canvas/Dock.tsx.
  import type { RelLabelMode } from "@mc/core/state/relLabels";

  export type Tool = "select" | "add" | "connect" | "layout";

  const REL_LABEL_GLYPH: Record<RelLabelMode, string> = { all: "≡", hidden: "⊘" };

  const REL_LABEL_OPTIONS: { mode: RelLabelMode; label: string; helper: string }[] = [
    { mode: "all", label: "Show labels", helper: "Multiplicities and roles on every relationship" },
    { mode: "hidden", label: "Hide all labels", helper: "Just the connector lines" },
  ];
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import type { ViewMode } from "@mc/core/state/viewMode";

  let {
    activeTool,
    onToolChange,
    viewMode,
    onToggleView,
    onClear,
    clearDisabled,
    relLabelMode = "all",
    onRelLabelModeChange,
  }: {
    activeTool: Tool;
    onToolChange: (tool: Tool) => void;
    viewMode: ViewMode;
    onToggleView: () => void;
    onClear: () => void;
    clearDisabled?: boolean;
    relLabelMode?: RelLabelMode;
    onRelLabelModeChange?: (mode: RelLabelMode) => void;
  } = $props();

  // Connect-button hover flyout: revealed after a 500ms hover delay; clicking
  // the button itself still activates the Connect tool immediately.
  let connectFlyoutOpen = $state(false);
  let connectFlyoutTimer: ReturnType<typeof setTimeout> | null = null;

  function clearConnectFlyoutTimer() {
    if (connectFlyoutTimer) {
      clearTimeout(connectFlyoutTimer);
      connectFlyoutTimer = null;
    }
  }
  function handleConnectEnter() {
    clearConnectFlyoutTimer();
    connectFlyoutTimer = setTimeout(() => {
      connectFlyoutOpen = true;
    }, 500);
  }
  function handleConnectLeave() {
    clearConnectFlyoutTimer();
    connectFlyoutOpen = false;
  }
  $effect(() => () => clearConnectFlyoutTimer());

  // Keyboard shortcuts V/N/C
  $effect(() => {
    function handler(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement).tagName;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
      if (e.key === "v") onToolChange("select");
      if (e.key === "n") onToolChange("add");
      if (e.key === "c") onToolChange("connect");
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
</script>

{#snippet dockTip(label: string)}
  <span
    class="pointer-events-none absolute left-[calc(100%+10px)] top-1/2 -translate-y-1/2 whitespace-nowrap rounded-md bg-slate-900 text-white text-[12px] font-medium px-2 py-1 opacity-0 -translate-x-1 group-hover:opacity-100 group-hover:translate-x-0 transition-all z-50 shadow-[0_6px_18px_rgba(15,23,42,0.28)]"
  >
    {label}
  </span>
{/snippet}

{#snippet selectIcon()}
  <svg viewBox="0 0 24 24" fill="currentColor" width="19" height="19">
    <path d="M4 3l7 17 2.5-6.5L20 11z" />
  </svg>
{/snippet}

{#snippet addIcon()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="19" height="19">
    <rect x="4" y="5" width="16" height="14" rx="2" />
    <path d="M12 9v6M9 12h6" />
  </svg>
{/snippet}

{#snippet connectIcon()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="19" height="19">
    <circle cx="6" cy="6" r="3" />
    <circle cx="18" cy="18" r="3" />
    <path d="M8.5 8.5l7 7" />
  </svg>
{/snippet}

{#snippet layoutIcon()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="19" height="19">
    <rect x="3" y="4" width="7" height="6" rx="1" />
    <rect x="14" y="4" width="7" height="6" rx="1" />
    <rect x="8" y="14" width="7" height="6" rx="1" />
    <path d="M6.5 10v2.5M17.5 10v2.5M11.5 12.5h-5M11.5 12.5h6" />
  </svg>
{/snippet}

{#snippet erdIcon()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="19" height="19">
    <rect x="3" y="4" width="8" height="16" rx="1" />
    <rect x="14" y="4" width="7" height="9" rx="1" />
    <path d="M11 8h3M7 9v6M17 13v3M17 16h-6" />
  </svg>
{/snippet}

{#snippet trashIcon()}
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    width="19"
    height="19"
  >
    <path d="M3 6h18" />
    <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
    <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
    <path d="M10 11v6M14 11v6" />
  </svg>
{/snippet}

{#snippet toolButton(icon: Snippet, tip: string, active: boolean, onClick: () => void)}
  <div class="relative group">
    <button
      onclick={onClick}
      aria-label={tip}
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {active
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
    >
      {@render icon()}
    </button>
    {@render dockTip(tip)}
  </div>
{/snippet}

<div
  data-dock
  class="absolute left-[14px] top-[calc(50%-34px)] -translate-y-1/2 bg-white border border-[#d8dee8] rounded-xl p-[6px] flex flex-col gap-1 z-20 shadow-[0_4px_16px_rgba(15,23,42,0.06)]"
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
>
  {@render toolButton(selectIcon, "Select & move (V)", activeTool === "select", () => onToolChange("select"))}
  {@render toolButton(
    addIcon,
    "Add object (N) — or double-click canvas",
    activeTool === "add",
    () => onToolChange("add"),
  )}

  <!-- Connect tool: click activates the tool; hover (after 500ms) reveals the
       relationship-labels flyout. These are separate concerns on one button. -->
  <div
    class="relative group"
    onmouseenter={handleConnectEnter}
    onmouseleave={handleConnectLeave}
    role="group"
  >
    <button
      onclick={() => onToolChange("connect")}
      aria-label="Connect (C) — or drag from a node's port"
      class="relative w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {activeTool ===
      'connect'
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
    >
      {@render connectIcon()}
      <span
        data-testid="rel-label-badge"
        aria-hidden="true"
        class="absolute -top-[3px] -right-[3px] min-w-[14px] h-[14px] px-[2px] rounded-full bg-slate-900 text-white text-[9px] leading-[14px] font-semibold text-center shadow-[0_1px_2px_rgba(15,23,42,0.4)]"
      >
        {REL_LABEL_GLYPH[relLabelMode]}
      </span>
    </button>

    {#if !connectFlyoutOpen}
      {@render dockTip("Connect (C) — or drag from a node's port")}
    {/if}

    {#if connectFlyoutOpen}
      <div class="absolute left-[calc(100%+10px)] top-1/2 -translate-y-1/2 z-50">
        <!-- invisible bridge so the cursor can travel from button to menu without closing -->
        <span class="absolute right-full top-0 h-full w-[12px]"></span>
        <div class="w-[260px] rounded-xl border border-[#d8dee8] bg-white p-1.5 shadow-[0_8px_24px_rgba(15,23,42,0.14)]">
          <div class="px-2 pt-1 pb-1.5 text-[11px] font-semibold uppercase tracking-wide text-slate-400">
            Relationship labels
          </div>
          {#each REL_LABEL_OPTIONS as opt (opt.mode)}
            {@const selected = opt.mode === relLabelMode}
            <button
              onclick={() => {
                onRelLabelModeChange?.(opt.mode);
                connectFlyoutOpen = false;
              }}
              class="flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left transition-colors {selected
                ? 'bg-[#e6f1fb]'
                : 'hover:bg-[#f1f3f7]'}"
            >
              <span
                class="mt-[1px] w-[16px] flex-shrink-0 text-center text-[12px] font-bold {selected
                  ? 'text-[#1e88e5]'
                  : 'text-slate-400'}"
              >
                {REL_LABEL_GLYPH[opt.mode]}
              </span>
              <span class="flex flex-col">
                <span class="text-[13px] font-semibold {selected ? 'text-[#1e88e5]' : 'text-slate-800'}">{opt.label}</span>
                <span class="text-[11px] leading-snug text-slate-500">{opt.helper}</span>
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
  {@render toolButton(layoutIcon, "Auto-layout (Dagre)", false, () => onToolChange("layout"))}

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
  <div class="relative group">
    <button
      onclick={onToggleView}
      aria-label="ERD view — show fields & field-level links"
      aria-pressed={viewMode === "erd"}
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {viewMode ===
      'erd'
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
    >
      {@render erdIcon()}
    </button>
    {@render dockTip(viewMode === "erd" ? "ERD view — fields & field-level links (on)" : "ERD view — show fields & field-level links")}
  </div>

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
  <div class="relative group">
    <button
      onclick={onClear}
      disabled={clearDisabled}
      aria-label="Clear canvas — delete everything"
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center transition-colors {clearDisabled
        ? 'bg-transparent text-slate-300 cursor-not-allowed'
        : 'bg-transparent text-slate-500 cursor-pointer hover:bg-[#fdf2f2] hover:text-[#dc2626]'}"
    >
      {@render trashIcon()}
    </button>
    {@render dockTip(clearDisabled ? "Clear canvas — nothing to clear" : "Clear canvas — delete everything")}
  </div>
</div>
