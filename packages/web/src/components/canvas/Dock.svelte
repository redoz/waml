<script module lang="ts">
  export type Tool = "select" | "add" | "connect" | "layout";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { Keyboard } from "lucide-svelte";
  import KeyHint from "../KeyHint.svelte";
  import { hints } from "../../state/hints.svelte";
  import { matchesShortcut, keyLabel } from "../../lib/shortcuts";

  let {
    activeTool,
    onToolChange,
    onClear,
    clearDisabled,
    onOpenProperties,
    leftOffset = 14,
  }: {
    activeTool: Tool;
    onToolChange: (tool: Tool) => void;
    onClear: () => void;
    clearDisabled?: boolean;
    // Opens the central edit panel's diagram-properties context.
    onOpenProperties?: () => void;
    // px from the canvas left edge; CanvasInner slides it right to clear the
    // docked navigator rail. Transitions so it glides rather than jumps.
    leftOffset?: number;
  } = $props();

  // Keyboard shortcuts, sourced from the registry so displayed glyphs and the
  // handled keys can never drift.
  $effect(() => {
    function handler(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement).tagName;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
      if (matchesShortcut("tool.select", e)) onToolChange("select");
      else if (matchesShortcut("tool.add", e)) onToolChange("add");
      else if (matchesShortcut("tool.connect", e)) onToolChange("connect");
      else if (matchesShortcut("hints.toggle", e)) hints.toggle();
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });

  // Reflect the toggle onto <html> so the global CSS reveals every .keyhint.
  $effect(() => {
    document.documentElement.toggleAttribute("data-show-shortcuts", hints.show);
    return () => document.documentElement.removeAttribute("data-show-shortcuts");
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

{#snippet slidersIcon()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" width="19" height="19">
    <path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h13M19 18h1" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="9" cy="12" r="2" />
    <circle cx="17" cy="18" r="2" />
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

{#snippet toolButton(icon: Snippet, tip: string, active: boolean, onClick: () => void, keys?: string[])}
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
    {#if keys}
      <KeyHint {keys} />
    {/if}
    {@render dockTip(tip)}
  </div>
{/snippet}

<div
  data-dock
  class="absolute top-[calc(50%-34px)] -translate-y-1/2 bg-white border border-[#d8dee8] rounded-xl p-[6px] flex flex-col gap-1 z-20 shadow-[0_4px_16px_rgba(15,23,42,0.06)] transition-[left] duration-200"
  style={`left: ${leftOffset}px; font-family: 'Source Sans 3 Variable', -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;`}
>
  {@render toolButton(selectIcon, "Select & move (V)", activeTool === "select", () => onToolChange("select"), keyLabel("tool.select"))}
  {@render toolButton(
    addIcon,
    "Add object (N) — or double-click canvas",
    activeTool === "add",
    () => onToolChange("add"),
    keyLabel("tool.add"),
  )}
  {@render toolButton(
    connectIcon,
    "Connect (C) — or drag from a node's port",
    activeTool === "connect",
    () => onToolChange("connect"),
    keyLabel("tool.connect"),
  )}

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
  {@render toolButton(layoutIcon, "Auto-layout (Dagre)", false, () => onToolChange("layout"))}

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>

  <!-- Diagram properties: opens the central edit panel's diagram-properties
       context, configuring the active diagram's per-diagram render settings. -->
  <div class="relative group">
    <button
      onclick={() => onOpenProperties?.()}
      aria-label="Diagram properties"
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900"
    >
      {@render slidersIcon()}
    </button>
    {@render dockTip("Diagram properties")}
  </div>

  <div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
  <div class="relative group">
    <button
      onclick={() => hints.toggle()}
      aria-label="Show keyboard shortcuts"
      aria-pressed={hints.show}
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {hints.show
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
    >
      <Keyboard size={19} />
    </button>
    <KeyHint keys={keyLabel("hints.toggle")} />
    {@render dockTip("Show keyboard shortcuts (?)")}
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
