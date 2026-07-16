<script lang="ts">
  // Dedicated host for the Inspector. Chosen over the generic ModelSheet because
  // the pin + translucent/hover-opaque behaviour is inspector-specific state that
  // would otherwise leak into the shared sheet (which still hosts the Share
  // panel). Provides its own resizable chrome, a pin toggle, and the
  // translucency logic.
  import type { Snippet } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { Pin, PinOff, ChevronUp, Pencil } from "lucide-svelte";
  import ElementPicker, { type Kind, KIND_ICON } from "./ElementPicker.svelte";

  // Combined slide + fade for the fold. Applied to a non-flex element so the
  // animated height actually takes (a flex-1 element ignores an animated height).
  function foldFade(node: HTMLElement, { duration = 200 } = {}) {
    const s = getComputedStyle(node);
    const height = parseFloat(s.height);
    const paddingTop = parseFloat(s.paddingTop);
    const paddingBottom = parseFloat(s.paddingBottom);
    return {
      duration,
      easing: cubicOut,
      css: (t: number) =>
        `overflow: hidden; opacity: ${t}; height: ${t * height}px;` +
        `padding-top: ${t * paddingTop}px; padding-bottom: ${t * paddingBottom}px;`,
    };
  }

  const MIN_WIDTH = 320;

  let {
    options,
    selectedKey,
    focusedKind,
    onSelect,
    pinned = false,
    onTogglePin,
    onEdit,
    hideDelay = 250,
    width = $bindable(380),
    children,
  }: {
    options: { key: string; label: string; kind: Kind }[];
    selectedKey: string | null;
    focusedKind: Kind | undefined;
    onSelect: (key: string | null, kind?: Kind) => void;
    pinned?: boolean;
    onTogglePin: () => void;
    /** Opens the edit dialog for the currently-focused element. */
    onEdit?: () => void;
    /** Delay (ms) before re-dimming after the pointer leaves — avoids flicker. */
    hideDelay?: number;
    width?: number;
    children?: Snippet;
  } = $props();

  // "engaged" = pointer over the panel or focus is inside it. A pinned panel is
  // translucent only while idle (not engaged); hover/focus fades it back opaque.
  let engaged = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  // Collapsed hides the body, leaving just the header bar. Local + not persisted.
  let collapsed = $state(false);
  // Whether any element is focused — drives body-vs-hint + collapse/icon affordances.
  const hasSelection = $derived(focusedKind !== undefined);

  const translucent = $derived(!pinned && !engaged);

  function engage() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
    }
    engaged = true;
  }

  function disengage() {
    if (hideTimer) clearTimeout(hideTimer);
    // Short delay so brushing past the edge doesn't flicker the panel.
    hideTimer = setTimeout(() => {
      engaged = false;
      hideTimer = undefined;
    }, hideDelay);
  }

  // ── Resize (left-edge drag), mirrors ModelSheet/Inspector behaviour ──────────
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
      const delta = startX - e.clientX;
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
</script>

<aside
  aria-label="Inspector"
  style={`width: ${width}px`}
  class={`absolute top-3 right-3 max-w-[calc(100%-24px)] max-h-[calc(100%-24px)] bg-white border border-[#d8dee8] rounded-xl overflow-hidden
    shadow-[0_8px_24px_rgba(15,23,42,0.14)] z-[16] flex flex-col transition-opacity duration-200 ${translucent ? "opacity-40" : "opacity-100"}`}
  onpointerenter={engage}
  onpointerleave={disengage}
  onfocusin={engage}
  onfocusout={disengage}
>
  <!-- Left-edge drag handle to resize (only when a body is shown) -->
  {#if hasSelection && !collapsed}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onmousedown={onResizeMouseDown}
      title="Drag to resize"
      class="absolute left-0 top-0 bottom-0 w-[6px] -ml-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
    ></div>
  {/if}

  <div class={`flex items-center gap-2 p-4 ${hasSelection && !collapsed ? "border-b border-[#d8dee8]" : ""}`}>
    {#if focusedKind}
      {@const KindIcon = KIND_ICON[focusedKind]}
      <span class="inspector-kind flex-none w-[26px] h-[26px] flex items-center justify-center rounded-md text-[#1e88e5] bg-[#e6f1fb]">
        <KindIcon size={15} />
      </span>
    {/if}
    <div class="flex-1 min-w-0">
      <ElementPicker {options} {selectedKey} {onSelect} />
    </div>
    {#if hasSelection}
      <button
        onclick={onEdit}
        aria-label="Edit element"
        title="Edit element"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <Pencil size={15} />
      </button>
    {/if}
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand inspector" : "Collapse inspector"}
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <span class={`flex transition-transform duration-200 ${collapsed ? "rotate-180" : ""}`}>
          <ChevronUp size={16} />
        </span>
      </button>
    {/if}
    <button
      onclick={onTogglePin}
      aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
      aria-pressed={pinned}
      title={pinned ? "Let it dim when idle" : "Keep solid"}
      class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
    >
      {#if pinned}
        <Pin size={16} />
      {:else}
        <PinOff size={16} />
      {/if}
    </button>
  </div>

  {#if hasSelection && !collapsed}
    <div class="flex-1 min-h-0 overflow-y-auto">
      <div transition:foldFade={{ duration: 200 }} class="p-4">
        {@render children?.()}
      </div>
    </div>
  {/if}
</aside>
