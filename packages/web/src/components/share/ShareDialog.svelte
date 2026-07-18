<script lang="ts">
  // Modal Share dialog. Replaces the old right-rail SharePanel: it hosts the
  // shareable link (read-only + Copy) and the "Share as image" flow (render the
  // current diagram to PNG, then Copy image / Save to disk with a live preview).
  import { Share2, Copy, ImageDown, Check, Clipboard, Download, X } from "lucide-svelte";

  let {
    shareUrl,
    imageName,
    canShareImage,
    generatePng,
    onClose,
  }: {
    /** The shareable link for the current model. */
    shareUrl: string;
    /** Base filename (no extension) for the saved PNG. */
    imageName: string;
    /** False for an empty diagram — disables the Share-as-image flow. */
    canShareImage: boolean;
    /** Produces the diagram PNG (SVG → canvas → blob); null if nothing to render. */
    generatePng: () => Promise<Blob | null>;
    onClose: () => void;
  } = $props();

  // Whether the browser can write an image to the clipboard (secure context +
  // ClipboardItem). Firefox historically lacks this, so Copy image is disabled
  // and the user falls back to Save / right-click on the preview.
  const clipboardImageSupported =
    typeof ClipboardItem !== "undefined" &&
    typeof navigator !== "undefined" &&
    !!navigator.clipboard &&
    typeof navigator.clipboard.write === "function";

  let linkCopied = $state(false);
  let rendering = $state(false);
  let renderError = $state<string | null>(null);
  let pngBlob = $state<Blob | null>(null);
  let previewUrl = $state<string | null>(null);
  let imageCopied = $state(false);

  // Revoke the object URL when it's replaced or the dialog unmounts.
  $effect(() => {
    const url = previewUrl;
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  });

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(shareUrl);
      linkCopied = true;
      setTimeout(() => (linkCopied = false), 2000);
    } catch {
      // Clipboard blocked (insecure context / permissions) — hand the user the
      // raw URL to copy manually.
      window.prompt("Copy this shareable link:", shareUrl);
    }
  }

  async function shareAsImage() {
    if (!canShareImage || rendering) return;
    rendering = true;
    renderError = null;
    try {
      const blob = await generatePng();
      if (!blob) {
        renderError = "Nothing to render yet.";
        return;
      }
      if (previewUrl) URL.revokeObjectURL(previewUrl);
      pngBlob = blob;
      previewUrl = URL.createObjectURL(blob);
    } catch {
      renderError = "Couldn't render the image — you can still export an SVG from the top bar.";
    } finally {
      rendering = false;
    }
  }

  async function copyImage() {
    if (!pngBlob || !clipboardImageSupported) return;
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": pngBlob })]);
      imageCopied = true;
      setTimeout(() => (imageCopied = false), 2000);
    } catch {
      renderError = "Couldn't copy the image — use Save instead.";
    }
  }

  function saveImage() {
    if (!pngBlob) return;
    const url = URL.createObjectURL(pngBlob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${imageName}.png`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/30"
  style="font-family: var(--font-ui);"
  onclick={onClose}
>
  <div
    role="dialog"
    aria-label="Share model"
    tabindex="-1"
    class="hud-surface relative w-[460px] max-w-[95vw] max-h-[88vh] flex flex-col overflow-hidden"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="relative z-[1] flex flex-col overflow-y-auto p-6">
    <!-- Header -->
    <div class="flex items-start justify-between gap-2 mb-4">
      <div class="flex items-center gap-3">
        <div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-[var(--round-chip)] bg-[color:rgba(var(--accent),.12)] text-[color:rgb(var(--accent))]">
          <Share2 size={16} />
        </div>
        <div>
          <div class="text-[15px] font-[700] text-[color:var(--ink)]">Named sharing</div>
          <div class="text-[12px] text-[color:rgb(var(--ink-faint))]">Share a model by name with a link</div>
        </div>
      </div>
      <button
        onclick={onClose}
        aria-label="Close"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--round-chip)] text-[color:rgb(var(--ink-faint))] hover:bg-[color:rgba(var(--accent),.12)] hover:text-[color:rgb(var(--accent))]"
      >
        <X size={18} />
      </button>
    </div>

    <!-- Share link -->
    <div class="flex gap-2 mb-5">
      <input
        type="text"
        value={shareUrl}
        readonly
        aria-label="Share URL"
        class="flex-1 min-w-0 rounded-[var(--round-chip)] border border-[color:var(--hair)] px-3 py-2.5 text-[13px] text-[color:var(--ink-dim)] bg-white outline-none select-all cursor-text focus:border-[color:rgb(var(--accent))] focus:ring-2 focus:ring-[color:rgba(var(--accent),.20)]"
      />
      <button
        onclick={copyLink}
        class="rounded-[var(--round-chip)] bg-[color:rgb(var(--accent))] px-4 py-2.5 text-[13px] font-[600] text-white hover:brightness-95 cursor-pointer flex-shrink-0 flex items-center gap-[6px]"
      >
        {#if linkCopied}<Check size={15} /> Copied{:else}<Copy size={15} /> Copy{/if}
      </button>
    </div>

    <div class="border-t border-[color:rgba(var(--accent),.14)] pt-4">
      <div class="text-[13px] font-[700] text-[color:var(--ink)] mb-1">Share as image</div>
      <div class="text-[12px] text-[color:rgb(var(--ink-faint))] mb-3">
        Render the current diagram to a PNG you can paste or save.
      </div>

      {#if !previewUrl}
        <button
          onclick={shareAsImage}
          disabled={!canShareImage || rendering}
          title={canShareImage ? "Render this diagram to a PNG" : "Add something to the diagram first"}
          class="flex w-full items-center justify-center gap-2 rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-white px-4 py-2.5 text-[14px] font-[600] text-[color:var(--ink)] hover:bg-[color:rgba(var(--accent),.10)] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <ImageDown size={16} />
          {rendering ? "Rendering…" : "Share as image"}
        </button>
      {:else}
        <!-- Preview — right-clickable so users can copy/save manually where the
             Clipboard API is unavailable (e.g. Firefox). -->
        <div class="mb-3 rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-[color:rgba(var(--accent),.06)] p-2 flex items-center justify-center overflow-hidden">
          <img src={previewUrl} alt="Diagram preview" class="max-h-[220px] max-w-full object-contain" />
        </div>
        <div class="flex gap-2">
          <button
            onclick={copyImage}
            disabled={!clipboardImageSupported}
            title={clipboardImageSupported ? "Copy the image to the clipboard" : "Your browser can't copy images — use Save or right-click the preview"}
            class="flex-1 flex items-center justify-center gap-2 rounded-[var(--round-chip)] bg-[color:rgb(var(--accent))] px-4 py-2.5 text-[13px] font-[600] text-white hover:brightness-95 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {#if imageCopied}<Check size={15} /> Copied{:else}<Clipboard size={15} /> Copy image{/if}
          </button>
          <button
            onclick={saveImage}
            class="flex-1 flex items-center justify-center gap-2 rounded-[var(--round-chip)] border border-[color:var(--hair)] bg-white px-4 py-2.5 text-[13px] font-[600] text-[color:var(--ink)] hover:bg-[color:rgba(var(--accent),.10)] cursor-pointer"
          >
            <Download size={15} /> Save
          </button>
        </div>
      {/if}

      {#if renderError}
        <div class="mt-3 text-[12px] text-[color:rgb(var(--danger))]">{renderError}</div>
      {:else if !canShareImage}
        <div class="mt-3 text-[12px] text-[color:rgb(var(--ink-faint))]">Add an object to the diagram to share it as an image.</div>
      {/if}
    </div>
    </div>
  </div>
</div>
