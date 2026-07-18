<script lang="ts">
  // Docked selection action bar: a floating pill fixed to the bottom-center of
  // the viewport (Figma-style). Presentational — the parent decides when to
  // mount it (selection non-empty + canvas hovered) and supplies the counts and
  // action callbacks. Fixed position means it never chases the selection,
  // never clips off the top edge, and never covers the elements it acts on.
  import { Trash2, LayoutDashboard } from "lucide-svelte";
  import { fly } from "svelte/transition";
  import KeyHint from "../KeyHint.svelte";
  import { keyLabel } from "../../lib/shortcuts";
  import { hudPress } from "../../lib/hudPress";

  let {
    nodeCount,
    edgeCount,
    onNewDiagram,
    onDelete,
  }: {
    nodeCount: number;
    edgeCount: number;
    onNewDiagram: (name: string) => void;
    onDelete: () => void;
  } = $props();

  // Inline-name mode for "New diagram from selection" (never window.prompt).
  let naming = $state(false);
  let name = $state("");

  const total = $derived(nodeCount + edgeCount);
  // Mixed / edges-only selection: a diagram needs at least one node member.
  const canCreate = $derived(nodeCount > 0);
  const summary = $derived(total === 1 ? "1 selected" : `${total} selected`);

  function startNaming() {
    naming = true;
    name = "";
  }
  function confirm() {
    const t = name.trim();
    if (!t) return; // reject empty / whitespace
    onNewDiagram(t);
    naming = false;
    name = "";
  }
  function cancel() {
    naming = false;
    name = "";
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      confirm();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    }
  }
</script>

<!-- Docked bottom-center. Slides up on appear so the link to the fresh
     selection reads. `nopan`/`nodrag` keep clicks from reaching the canvas
     underneath. Fixed → positions against the viewport, not the selection. -->
<div
  data-testid="selection-toolbar"
  class="nopan nodrag st-dock"
  transition:fly={{ y: 12, duration: 150 }}
>
  <div class="hud-surface st-pill">
    {#if naming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        aria-label="New diagram name"
        bind:value={name}
        onkeydown={onKey}
        placeholder="New diagram name"
        autofocus
        class="st-input"
      />
      <button use:hudPress onclick={confirm} aria-label="Create diagram" class="hud-surface hud-surface--btn hud-btn hud-btn--sm">Create diagram</button>
      <button onclick={cancel} aria-label="Cancel" class="st-text">Cancel</button>
    {:else}
      <span class="st-summary">{summary}</span>
      <div class="st-sep"></div>
      <button
        use:hudPress
        onclick={startNaming}
        disabled={!canCreate}
        aria-label="New diagram from selection"
        title={canCreate ? "New diagram seeded with the selected objects" : "Select at least one object to create a diagram"}
        class="hud-surface hud-surface--btn hud-btn hud-btn--sm st-action"
      >
        <LayoutDashboard size={14} /> New diagram from selection
      </button>
      <button
        use:hudPress
        onclick={onDelete}
        aria-label="Delete selection"
        title="Delete the selected objects and relationships"
        class="hud-surface hud-surface--btn hud-btn hud-btn--sm st-action st-danger"
        style="--accent: var(--danger)"
      >
        <Trash2 size={14} /> Delete selection
        <KeyHint keys={keyLabel("selection.delete")} />
      </button>
    {/if}
  </div>
</div>

<style>
  .st-dock { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%); z-index: 30; }
  .st-pill { display: flex; align-items: center; gap: 6px; padding: 6px; }
  .st-summary { padding: 0 8px; font: 500 12px/1 var(--font-ui); color: rgb(var(--ink-faint)); white-space: nowrap; }
  .st-sep { width: 1px; height: 20px; background: rgba(var(--accent), .18); }
  /* compact hud-btn overrides for the toolbar: real text label, not wide caps */
  .st-action, .st-danger { display: inline-flex; align-items: center; gap: 6px; text-transform: none; letter-spacing: .02em; font-weight: 600; font-family: var(--font-ui); font-size: 12px; }
  .st-input {
    width: 180px; font: 500 13px/1 var(--font-ui); padding: 6px 8px; color: var(--ink);
    background: #fff; border: 1px solid rgba(var(--accent), .26); border-radius: 3px; outline: 0;
  }
  .st-input:focus { border-color: rgb(var(--accent)); box-shadow: 0 0 0 1px rgb(var(--accent)); }
  .st-text { border: 0; background: transparent; cursor: pointer; padding: 7px 10px; border-radius: 2px; font: 500 12px/1 var(--font-ui); color: rgb(var(--ink-faint)); }
  .st-text:hover { background: rgba(var(--accent), .12); color: rgb(var(--accent)); }
  /* Restore the Del-shortcut hover reveal lost when this button dropped
     Tailwind's `group` hook (KeyHint is opacity-0 until revealed). */
  .st-danger:hover :global(.keyhint) { opacity: 1; }
</style>
