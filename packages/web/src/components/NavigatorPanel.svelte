<script lang="ts">
  // Two-mode host for the model navigator. Unpinned = centered modal over a
  // dismissing scrim (CentralEditPanel treatment). Pinned = left-docked rail
  // mirroring InspectorPanel (no scrim, right-edge resize, translucent-when-idle,
  // collapsible). The pin button toggles the two modes; Esc / scrim / close
  // dismiss. State is owned by the caller (CanvasInner) and passed in.
  import { Pin, PinOff, PanelLeft, ChevronUp } from "lucide-svelte";
  import NavigatorBody from "./NavigatorBody.svelte";
  import type { ModelGraph } from "@waml/okf";
  import type { NavKind } from "@waml/core/nav/tree";

  const MIN_WIDTH = 300;

  let {
    open,
    mode,
    width = $bindable(340),
    collapsed = $bindable(false),
    title,
    onClose,
    onToggleMode,
    pinned = false,
    onTogglePin,
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
    open: boolean;
    mode: "centered" | "docked";
    width?: number;
    collapsed?: boolean;
    title: string;
    onClose: () => void;
    onToggleMode: () => void;
    pinned?: boolean;
    onTogglePin: () => void;
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

  // Bundle the body passthrough once so both mode branches stay DRY.
  const body = $derived({
    graph, scopeKey, activeDiagramKey, palette,
    onScope, onSelectDiagram, onReorder, onViewInDiagram, onAddToNewDiagram,
    onEditProperties, onCreatePackage, onCreateNode, onCreateDiagram,
    onRename, onSort, onDelete,
  });

  let card = $state<HTMLElement | null>(null);

  // Docked translucency: solid while engaged (pointer over / focus inside), dim
  // when idle. Mirrors InspectorPanel.
  let engaged = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  const translucent = $derived(mode === "docked" && !pinned && !engaged);
  function engage() {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = undefined; }
    engaged = true;
  }
  function disengage() {
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => { engaged = false; hideTimer = undefined; }, 250);
  }

  // Right-edge resize (mirror of InspectorPanel's left-edge drag: dragging right
  // widens, so delta = current - start).
  let resizing = false;
  let startX = 0;
  let startWidth = 0;
  function onResizeMouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    startX = e.clientX;
    startWidth = width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }
  $effect(() => {
    function onMouseMove(e: MouseEvent) {
      if (!resizing) return;
      const delta = e.clientX - startX;
      width = Math.min(window.innerWidth * 0.6, Math.max(MIN_WIDTH, startWidth + delta));
    }
    function onMouseUp() {
      if (!resizing) return;
      resizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    }
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  });

  // Unified Esc: while open, first Esc blurs a focused inner input (protecting an
  // in-progress inline create/rename), otherwise closes. Works in both modes.
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (!open || e.key !== "Escape") return;
      const active = document.activeElement as HTMLElement | null;
      const editing =
        !!active && ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName);
      if (editing) {
        active!.blur();
        e.stopPropagation();
      } else {
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Move focus into the centered card on open so it reads as a dialog.
  $effect(() => {
    if (open && mode === "centered") card?.focus();
  });
</script>

{#snippet header(docked: boolean)}
  <div class="px-4 py-[13px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0 bg-white">
    <h2 class="text-[14px] font-[700] flex-1 min-w-0 truncate text-slate-900">{title}</h2>
    {#if docked}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand navigator" : "Collapse navigator"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand navigator" : "Collapse navigator"}
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <span class={`flex transition-transform duration-200 ${collapsed ? "rotate-180" : ""}`}>
          <ChevronUp size={16} />
        </span>
      </button>
      <button
        onclick={onTogglePin}
        aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
        aria-pressed={pinned}
        title={pinned ? "Let it dim when idle" : "Keep solid"}
        class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
      >
        {#if pinned}<Pin size={16} />{:else}<PinOff size={16} />{/if}
      </button>
    {:else}
      <button
        onclick={onToggleMode}
        aria-label="Pin navigator to left"
        aria-pressed={false}
        title="Pin navigator to left"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors text-slate-500 hover:bg-[#f1f3f7]"
      >
        <PanelLeft size={16} />
      </button>
    {/if}
    <button
      onclick={onClose}
      aria-label="Close"
      title="Close"
      class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7] text-[20px] leading-none"
    >
      ×
    </button>
  </div>
{/snippet}

{#if open}
  {#if mode === "centered"}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      data-testid="nav-scrim"
      onclick={onClose}
      class="fixed inset-0 z-[60] flex items-center justify-center bg-slate-900/30 p-4"
      style="font-family: 'Source Sans 3 Variable', -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        bind:this={card}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        class="relative w-full max-w-[620px] h-[95vh] max-h-[95vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)] overflow-hidden"
      >
        {@render header(false)}
        <div class="flex-1 min-h-0 overflow-hidden">
          <NavigatorBody {...body} />
        </div>
      </div>
    </div>
  {:else}
    <aside
      aria-label="Model navigator"
      style={`width: ${width}px`}
      class={`absolute top-3 left-3 max-w-[calc(100%-24px)] max-h-[calc(100%-24px)] bg-white border border-[#d8dee8] rounded-xl overflow-hidden shadow-[0_8px_24px_rgba(15,23,42,0.14)] z-[16] flex flex-col transition-opacity duration-200 ${translucent ? "opacity-40" : "opacity-100"}`}
      onpointerenter={engage}
      onpointerleave={disengage}
      onfocusin={engage}
      onfocusout={disengage}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        data-testid="nav-resize"
        onmousedown={onResizeMouseDown}
        title="Drag to resize"
        class="absolute right-0 top-0 bottom-0 w-[6px] -mr-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
      ></div>
      {@render header(true)}
      {#if !collapsed}
        <div class="flex-1 min-h-0 overflow-hidden">
          <NavigatorBody {...body} />
        </div>
      {/if}
    </aside>
  {/if}
{/if}
