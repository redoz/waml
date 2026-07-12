# UAML LSP — Formatting + Buffer Lifecycle

**Date:** 2026-07-12
**Status:** Design approved

## Problem

Two independent gaps in the language server, neither needing the spanned-syntax
foundation:

1. **No formatting.** UAML has a canonical form — `serialize_document` is the
   formatter, exposed as the `uaml fmt` CLI — but the LSP advertises no
   `document_formatting_provider`, so "Format Document" does nothing.

2. **Stale open-buffer overlays.** The `Workspace` is one `HashMap<path,
   text>` shared by disk seeds and open-buffer overlays (buffers win via
   `entry().or_insert`). `didOpen`/`didChange` overlay a buffer's text, but
   there is **no `didClose` and no `didSave`**. When a document is closed, its
   last in-memory overlay lives forever — cross-file checks keep validating
   against buffer text that no longer matches disk. An externally-rewritten
   file (git checkout, a formatter) is never reconciled.

## Goals

- `textDocument/formatting` returns the canonical form, reusing the exact
  parse→serialize path the `fmt` CLI uses, including its data-loss guard.
- `didClose` reverts a closed buffer to its on-disk state.
- `didSave` reconciles the saved file from disk.
- Zero changes to the `uaml` core crate. Pure LSP-crate wiring.

## Design

### 1. Formatting

Advertise `document_formatting_provider: Some(OneOf::Left(true))` in
`initialize`.

Handler `textDocument/formatting`:

- Resolve the request's document path; read its current text from the
  `Workspace` overlay.
- Reuse **`commands::plan_fmt`** on that single `(path, text)` pair — the same
  function `uaml fmt` calls. It returns `FmtResult { formatted, changed,
  skipped }`.
- **`skipped`** is the critical safety property: `plan_fmt` skips any file
  whose `validate` reports an **Error-severity** diagnostic, preserving it
  byte-for-byte. A malformed / mid-edit buffer is therefore never rewritten
  (no data loss from a lossy round-trip). This guard comes for free by reusing
  `plan_fmt`.
- If `changed && !skipped`: return a **single full-document `TextEdit`** whose
  range spans the whole document (line 0, col 0) → (last line, last col), with
  `new_text = formatted`. VS Code computes the minimal visual diff from that.
- Otherwise return no edits (`None` / empty).

`serialize` is a proven idempotent fixpoint (golden test + the wasm
`fmt_is_idempotent` test), so formatting never thrashes.

Range formatting (`rangeFormatting`) is **out of scope** — UAML formats whole
documents; partial-range formatting has no meaning here.

### 2. Buffer lifecycle

Two new `Workspace` methods plus handler wiring in `server.rs`.

```rust
impl Workspace {
    /// Closed buffer: revert to on-disk state. Re-read the file; on success
    /// overlay the disk text, on failure (deleted) drop the entry.
    pub fn close(&mut self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(disk) => { self.docs.insert(path.into(), disk); }
            Err(_)   => { self.docs.remove(path); }
        }
    }

    /// Saved buffer: reconcile from disk (catches format-on-save / external
    /// rewrite). Same read-and-overlay; on read failure keep the buffer.
    pub fn save_reconcile(&mut self, path: &str) {
        if let Ok(disk) = std::fs::read_to_string(path) {
            self.docs.insert(path.into(), disk);
        }
    }
}
```

- `textDocument/didClose` → `Workspace.close(path)` → `publish_all()`. The
  closed file's diagnostics now reflect disk; if deleted, it leaves the bundle
  and its cross-file references resolve against the remaining docs.
- `textDocument/didSave` → `Workspace.save_reconcile(path)` → `publish_all()`.

Path normalization matches `didOpen`/`didChange`: URI → file path →
`replace('\\', "/")`.

Advertising: `did_close`/`did_save` are notifications under the existing
`TextDocumentSyncKind::FULL`; no capability change is strictly required, but
set `text_document_sync` to a `TextDocumentSyncOptions` with `save:
Some(..)` and `open_close: true` so the client reliably sends both.

## Data flow

```
didClose { uri }  → close(path): disk ? overlay(disk) : remove   → publish_all
didSave  { uri }  → save_reconcile(path): disk ? overlay(disk)   → publish_all
formatting { uri } → plan_fmt(path, overlay_text)
                     → changed && !skipped ? [full-doc TextEdit] : none
```

## Error handling / edge cases

- **Format a malformed buffer** — `plan_fmt` marks it `skipped`; handler
  returns no edits. Never rewrites broken content.
- **Format an unchanged buffer** — `changed == false`; no edits.
- **Close a never-saved / deleted file** — `read_to_string` errors; entry is
  removed; bundle stays consistent.
- **Save reconcile read failure** — keep the existing buffer (don't blank a
  doc on a transient read error).
- **Non-UAML document** — `plan_fmt` operates per file; a plain-markdown file
  round-trips to itself (`changed == false`) → no edits. Consistent with the
  existing `is_uaml` diagnostic filter.

## Testing

**`Workspace` unit (`bundle.rs`):**
- `close` reverts an overlaid buffer to disk text (write a temp file, overlay
  a divergent buffer, `close`, assert diagnostics reflect disk).
- `close` on a deleted file removes the entry.
- `save_reconcile` picks up an externally-rewritten file.

**Formatting unit (`commands.rs` already covers `plan_fmt`):**
- LSP handler test: a loosely-spaced valid doc → one full-doc `TextEdit` with
  canonical text; an unchanged doc → no edits; an Error-diagnostic doc →
  `skipped` → no edits.

**e2e (`lsp_e2e.rs`):**
- initialize advertises `documentFormattingProvider`.
- `textDocument/formatting` on a messy fixture returns the canonical edit.
- `didClose` after a divergent `didChange` reverts published diagnostics to
  the disk state.

## Non-goals

- `rangeFormatting`, format-on-type.
- `willSaveWaitUntil` (format-on-save handshakes).
- File-watching / `workspace/didChangeWatchedFiles` (external change detection
  beyond save/close reconcile) — later, if needed.
- Any core-crate change.

## Roadmap position

Phase 1b of the LSP build-up — independent of the spanned-syntax foundation
(Phase 1a). Shippable on its own, in either order.
