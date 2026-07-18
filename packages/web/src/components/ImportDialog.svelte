<script lang="ts">
  // Mirrors packages/web/src/components/ImportDialog.tsx.
  import { Copy, Check, X } from "lucide-svelte";
  import { parsePastedMarkdown, zipToFiles } from "@waml/core/okf/io";
  import { build_model } from "@waml/wasm";

  type Bundle = [string, string][];

  let { onConfirm, onClose }: {
    onConfirm: (bundle: Bundle, mode: "replace" | "merge") => void;
    onClose: () => void;
  } = $props();

  let pasteText = $state("");
  let error: string | null = $state(null);
  let preview: { bundle: Bundle; nodes: number; edges: number } | null = $state(null);
  let mode: "replace" | "merge" = $state("replace");
  let copied = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();

  const MARKER_RE = /<!--\s*.+?\s*-->\n/;

  // Copy the AI authoring guide to the clipboard so the user can paste it into
  // Claude/ChatGPT to generate an importable OKF model. Falls back to opening
  // the raw guide if the clipboard is blocked.
  async function copyInstructions() {
    try {
      const md = await fetch("/okf-format.md").then(r => r.text());
      await navigator.clipboard.writeText(md);
      copied = true;
      setTimeout(() => { copied = false; }, 2500);
    } catch {
      window.open("/okf-format.md", "_blank");
    }
  }

  // Collect the current inputs into an OKF bundle (`[path, markdown][]`). A file
  // or paste that is itself a concatenated bundle (HTML-comment path markers) is
  // expanded into its constituent documents. Throws on empty input.
  async function buildBundle(paste: string): Promise<Bundle> {
    let files: Record<string, string> = {};
    const uploaded = fileInput?.files;
    if (uploaded && uploaded.length > 0) {
      for (const file of Array.from(uploaded)) {
        if (file.name.endsWith(".zip")) {
          Object.assign(files, zipToFiles(new Uint8Array(await file.arrayBuffer())));
        } else {
          const text = await file.text();
          if (MARKER_RE.test(text)) Object.assign(files, parsePastedMarkdown(text));
          else files[file.name] = text;
        }
      }
    }
    if (paste.trim()) files = { ...files, ...parsePastedMarkdown(paste.trim()) };
    if (Object.keys(files).length === 0) throw new Error("Provide a file or paste markdown content.");
    return Object.entries(files);
  }

  // Re-parse to drive the live preview/count. Empty input clears both; a parse
  // error is shown (and clears the preview) so the count never lies. Counts come
  // from the WASM core (`build_model`).
  async function refresh(paste: string) {
    const hasInput = (fileInput?.files?.length ?? 0) > 0 || paste.trim().length > 0;
    if (!hasInput) { preview = null; error = null; return; }
    try {
      const bundle = await buildBundle(paste);
      const m = build_model(bundle) as { nodes: unknown[]; edges: unknown[] };
      preview = { bundle, nodes: m.nodes.length, edges: m.edges.length };
      error = null;
    } catch (e) { preview = null; error = (e as Error).message ?? "Failed to parse OKF bundle."; }
  }
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30"
  style="font-family: var(--font-ui);"
  onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
>
  <div
    class="hud-surface relative w-[480px] max-w-[95vw] flex flex-col overflow-hidden"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="relative z-[1] flex flex-col gap-4 p-6">
    <div class="flex items-center justify-between">
      <h2 class="text-[15px] font-semibold text-[color:var(--ink)]">Import OKF bundle</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <!-- Generate a model with AI: copy the authoring guide → paste into
         Claude/ChatGPT. The raw guide also lives at /okf-format.md so an
         assistant can fetch it directly. -->
    <div class="-mt-1 flex flex-col gap-1.5 rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-[color:rgba(var(--accent),.06)] px-3 py-2.5">
      <span class="text-[12.5px] text-[color:var(--ink-dim)]">No model yet? Generate one with AI:</span>
      <div class="flex flex-wrap items-center gap-3">
        <button
          onclick={copyInstructions}
          class="flex items-center gap-[6px] rounded-[var(--round-chip)] bg-[color:rgb(var(--accent))] px-3 py-[6px] text-[12.5px] font-[600] text-white hover:brightness-95"
        >
          {#if copied}
            <Check size={14} /> Copied — paste into Claude
          {:else}
            <Copy size={14} /> Copy AI instructions
          {/if}
        </button>
        <a
          href="/okf-format.md"
          target="_blank"
          rel="noopener"
          class="text-[12.5px] text-[color:rgb(var(--accent))] hover:underline underline-offset-2"
        >
          View guide ↗
        </a>
      </div>
    </div>

    <!-- File upload -->
    <div>
      <label class="block text-[13px] font-medium text-[color:var(--ink-dim)] mb-1" for="import-file-input">
        Upload .md / .txt / .zip files
      </label>
      <input
        id="import-file-input"
        bind:this={fileInput}
        type="file"
        accept=".md,.txt,.zip"
        multiple
        onchange={() => void refresh(pasteText)}
        class="block w-full text-[13px] text-[color:rgb(var(--ink-faint))] file:mr-3 file:py-1 file:px-3 file:rounded-[var(--round-chip)] file:border file:border-[color:var(--hair)] file:bg-white file:text-[13px] file:font-medium file:cursor-pointer file:text-[color:var(--ink)] hover:file:bg-[color:rgba(var(--accent),.10)]"
      />
    </div>

    <!-- Paste area -->
    <div>
      <label class="block text-[13px] font-medium text-[color:var(--ink-dim)] mb-1" for="import-paste-area">
        Or paste markdown content
      </label>
      <textarea
        id="import-paste-area"
        value={pasteText}
        oninput={(e) => { pasteText = e.currentTarget.value; void refresh(e.currentTarget.value); }}
        placeholder={"<!-- path/to/file.md -->\n...content..."}
        rows={6}
        style="font-family: var(--font-mono);"
        class="w-full text-[13px] text-[color:var(--ink)] border border-[color:var(--hair)] rounded-[var(--round-chip)] px-3 py-2 resize-none focus:outline-none focus:ring-2 focus:ring-[color:rgb(var(--accent))]"
      ></textarea>
    </div>

    {#if error}
      <p class="text-[13px] text-[color:rgb(var(--danger))] bg-[color:rgba(var(--danger),.10)] border border-[color:rgba(var(--danger),.30)] rounded-[var(--round-chip)] px-3 py-2">
        {error}
      </p>
    {/if}

    <!-- Apply mode + count — mirrors the OKF import dialog -->
    {#if preview}
      <div class="flex flex-col gap-1.5 border-t border-[color:rgba(var(--accent),.14)] pt-3">
        <span class="text-[12px] font-medium text-[color:rgb(var(--ink-faint))]">When applying to the canvas</span>
        {#each (["replace", "merge"] as const) as m (m)}
          <label class="flex items-center gap-2 text-[13px] text-[color:var(--ink)] cursor-pointer">
            <input type="radio" name="okf-mode" checked={mode === m} onchange={() => { mode = m; }} style="accent-color: rgb(var(--accent));" />
            {m === "replace" ? "Replace the canvas" : "Merge into the canvas"}
          </label>
        {/each}
        <p class="text-[12px] text-[color:rgb(var(--ink-faint))]">
          Will import {preview.nodes} nodes, {preview.edges} relationships.
        </p>
      </div>
    {/if}

    <div class="flex gap-2 justify-end">
      <button
        onclick={onClose}
        class="text-[13px] font-[600] border border-[color:var(--hair)] bg-white text-[color:var(--ink)] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:bg-[color:rgba(var(--accent),.10)]"
      >
        Cancel
      </button>
      <button
        onclick={() => preview && onConfirm(preview.bundle, mode)}
        disabled={!preview}
        class="text-[13px] font-[600] bg-[color:rgb(var(--accent))] text-white border border-[color:rgb(var(--accent))] rounded-[var(--round-chip)] px-4 py-[7px] cursor-pointer hover:brightness-95 disabled:opacity-50"
      >
        Import
      </button>
    </div>
    </div>
  </div>
</div>
