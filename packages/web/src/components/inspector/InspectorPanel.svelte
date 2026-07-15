<script lang="ts">
  // Dedicated host for the Inspector. Chosen over the generic ModelSheet because
  // the pin + translucent/hover-opaque behaviour is inspector-specific state that
  // would otherwise leak into the shared sheet (which still hosts the Share
  // panel). Provides its own resizable chrome, a pin toggle, and the
  // translucency logic.
  import type { Snippet } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { Pin, PinOff, ChevronUp, ChevronDown, Box, Spline, Pencil, Check } from "lucide-svelte";

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
    options: { key: string; label: string }[];
    selectedKey: string | null;
    focusedKind: "node" | "edge" | undefined;
    onSelect: (key: string | null) => void;
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

  const translucent = $derived(pinned && !engaged);

  // ── Element picker (custom listbox) ──────────────────────────────────────────
  // Replaces a native <select> so the option list can carry the same styling as
  // the diagram/object switcher (Navigator). The menu is position:fixed so it
  // escapes the panel's overflow-hidden clip; coordinates are measured off the
  // trigger when it opens.
  let open = $state(false);
  let highlighted = $state(-1);
  let triggerEl: HTMLButtonElement | undefined;
  let menuStyle = $state("");

  const selectedLabel = $derived(options.find((o) => o.key === selectedKey)?.label);

  function openMenu() {
    if (triggerEl) {
      const r = triggerEl.getBoundingClientRect();
      menuStyle = `left: ${r.left}px; top: ${r.bottom + 6}px; min-width: ${r.width}px;`;
    }
    highlighted = Math.max(0, options.findIndex((o) => o.key === selectedKey));
    open = true;
  }

  function closeMenu() {
    open = false;
  }

  function toggleMenu() {
    if (open) closeMenu();
    else openMenu();
  }

  function choose(key: string) {
    onSelect(key);
    closeMenu();
    triggerEl?.focus();
  }

  function onTriggerKeydown(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        openMenu();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      closeMenu();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      highlighted = Math.min(options.length - 1, highlighted + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlighted = Math.max(0, highlighted - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (highlighted >= 0 && options[highlighted]) choose(options[highlighted].key);
    }
  }

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
      <span class="inspector-kind flex-none w-[26px] h-[26px] flex items-center justify-center rounded-md text-[#1e88e5] bg-[#e6f1fb]">
        {#if focusedKind === "node"}
          <Box size={15} />
        {:else}
          <Spline size={15} />
        {/if}
      </span>
    {/if}
    <div class="flex-1 min-w-0">
      <button
        bind:this={triggerEl}
        type="button"
        role="combobox"
        aria-label="Select element"
        aria-haspopup="listbox"
        aria-controls="inspector-element-listbox"
        aria-expanded={open}
        onclick={toggleMenu}
        onkeydown={onTriggerKeydown}
        class="w-full flex items-center gap-1.5 min-w-0 text-[14px] rounded-md py-1 px-1.5 cursor-pointer transition-colors hover:bg-[#f1f3f7] focus:outline-none focus:ring-2 focus:ring-[#e6f1fb]"
      >
        <span
          class={`flex-1 truncate text-left ${selectedLabel ? "font-semibold text-slate-900" : "font-medium text-slate-400"}`}
        >
          {selectedLabel ?? "Select an element…"}
        </span>
        <ChevronDown
          size={15}
          class={`flex-none text-slate-400 transition-transform duration-150 ${open ? "rotate-180" : ""}`}
        />
      </button>

      {#if open}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="fixed inset-0 z-[59]" onclick={closeMenu}></div>
        <div
          id="inspector-element-listbox"
          role="listbox"
          aria-label="Select element"
          tabindex="-1"
          style={menuStyle}
          class="fixed z-[60] max-h-[280px] overflow-y-auto rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1"
        >
          {#if options.length === 0}
            <div class="px-3 py-2 text-[13px] text-slate-400">No elements in this diagram</div>
          {/if}
          {#each options as opt, i (opt.key)}
            <button
              type="button"
              role="option"
              aria-selected={opt.key === selectedKey}
              onclick={() => choose(opt.key)}
              onmouseenter={() => (highlighted = i)}
              class={`w-full text-left px-3 py-2 text-[13px] cursor-pointer flex items-center gap-[7px] ${
                i === highlighted ? "bg-[#f1f3f7]" : ""
              } ${opt.key === selectedKey ? "text-[#1e88e5] font-[600]" : "text-slate-900"}`}
            >
              <Box
                size={14}
                class={`flex-shrink-0 ${opt.key === selectedKey ? "text-[#1e88e5]" : "text-slate-400"}`}
              />
              <span class="truncate">{opt.label}</span>
              {#if opt.key === selectedKey}
                <Check size={14} class="ml-auto flex-shrink-0 text-[#1e88e5]" />
              {/if}
            </button>
          {/each}
        </div>
      {/if}
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
      aria-label={pinned ? "Unpin inspector" : "Pin inspector"}
      aria-pressed={pinned}
      title={pinned ? "Unpin inspector" : "Pin inspector"}
      class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
    >
      {#if pinned}
        <PinOff size={16} />
      {:else}
        <Pin size={16} />
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
