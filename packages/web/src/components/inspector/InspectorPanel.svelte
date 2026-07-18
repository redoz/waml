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

  // "engaged" = pointer over the panel or focus is inside it. An unpinned panel is
  // translucent only while idle (not engaged); hover/focus fades it back opaque; a pinned panel stays solid.
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
  style={`width: ${width}px; ${translucent ? "opacity:.4" : "opacity:1"}`}
  class="hud-surface insp-panel"
  onpointerenter={engage}
  onpointerleave={disengage}
  onfocusin={engage}
  onfocusout={disengage}
>
  <!-- Left-edge drag handle to resize (only when a body is shown) -->
  {#if hasSelection && !collapsed}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div onmousedown={onResizeMouseDown} title="Drag to resize" class="insp-resize"></div>
  {/if}

  <div class={`insp-head ${hasSelection && !collapsed ? "insp-head--divide" : ""}`}>
    {#if focusedKind}
      {@const KindIcon = KIND_ICON[focusedKind]}
      <span class="insp-kind"><KindIcon size={15} /></span>
    {/if}
    <div style="flex:1;min-width:0"><ElementPicker {options} {selectedKey} {onSelect} /></div>
    {#if hasSelection}
      <button onclick={onEdit} aria-label="Edit element" title="Edit element" class="insp-iconbtn"><Pencil size={15} /></button>
    {/if}
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand inspector" : "Collapse inspector"}
        class="insp-iconbtn"
      >
        <span class="insp-caret" style={collapsed ? "transform:rotate(180deg)" : ""}><ChevronUp size={16} /></span>
      </button>
    {/if}
    <button
      onclick={onTogglePin}
      aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
      aria-pressed={pinned}
      title={pinned ? "Let it dim when idle" : "Keep solid"}
      class={`insp-iconbtn ${pinned ? "is-active" : ""}`}
    >
      {#if pinned}<Pin size={16} />{:else}<PinOff size={16} />{/if}
    </button>
  </div>

  {#if hasSelection && !collapsed}
    <div class="insp-body">
      <div transition:foldFade={{ duration: 200 }} class="insp-body-inner">
        {@render children?.()}
      </div>
    </div>
  {/if}
</aside>

<style>
  .insp-panel {
    position: absolute; top: 12px; right: 12px;
    max-width: calc(100% - 24px); max-height: calc(100% - 24px);
    overflow: hidden; z-index: 16;
    display: flex; flex-direction: column;
    transition: opacity .2s ease;
  }
  .insp-resize { position: absolute; left: 0; top: 0; bottom: 0; width: 6px; margin-left: -3px; cursor: col-resize; z-index: 17; }
  .insp-resize:hover { background: rgba(var(--accent), .20); }
  .insp-head { display: flex; align-items: center; gap: 8px; padding: 12px; position: relative; z-index: 1; }
  .insp-head--divide { border-bottom: 1px solid rgba(var(--accent), .22); }
  .insp-kind {
    flex: none; width: 26px; height: 26px; display: grid; place-items: center;
    border-radius: 2px; color: rgb(var(--accent)); background: rgba(var(--accent), .12);
  }
  .insp-iconbtn {
    width: 30px; height: 30px; display: grid; place-items: center; border: 0; background: transparent;
    border-radius: 2px; color: rgb(var(--ink-faint)); cursor: pointer;
  }
  .insp-iconbtn:hover { background: rgba(var(--accent), .12); color: rgb(var(--accent)); }
  .insp-iconbtn.is-active { color: rgb(var(--accent)); background: rgba(var(--accent), .12); }
  .insp-body { flex: 1; min-height: 0; overflow-y: auto; position: relative; z-index: 1; }
  .insp-body-inner { padding: 16px; }
  .insp-caret { display: flex; transition: transform .2s ease; }
</style>
