# View Source Markdown Render Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render a classifier subject's markdown in the already-wired View Source tab, replacing its empty placeholder slot.

**Architecture:** The App owns the raw on-disk bundle (`self.bundle: Vec<(rel_path, contents)>`) and already toggles the Source tab's body slot in `sync_active_tab`. Add a pure key→source lookup helper, swap the opaque placeholder slot for a scrolling `Markdown` widget, and feed the subject's raw `.md` text into it when a Source tab is active. No `DocView` trait change; both context-menu triggers (tree row + diagram node) already open the Source tab.

**Tech Stack:** Rust, makepad (redoz fork, current with upstream), upstream `Markdown` widget (`makepad-widgets/src/markdown.rs`).

## Global Constraints

- Never edit the main checkout directly — implement in a git worktree (redoz@ rule).
- Fork parity: upstream makepad `Markdown` widget is available; DSL name `Markdown`, accessor `WidgetRef::as_markdown()`, setter `MarkdownRef::set_text(cx, &str)`.
- Bundle paths use forward slashes and may be nested (`pkg/order.md`); subject `key` is the bare slug (`order`).
- Per-pid visual verification is mandatory for UI changes — capture by specific pid, never kill-all the user's editor.
- Source text is the **raw bundle file**, verbatim — not a `serialize_document` re-render.

---

### Task 1: `source_for` bundle lookup helper

Pure function: given the bundle and a subject key, return the raw `.md` contents whose file basename (minus `.md`) equals the key. Lives beside the other bundle code in `load.rs`.

**Files:**
- Modify: `crates/waml-editor/src/load.rs` (add `source_for` + tests near the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing new. Bundle shape is `&[(String, String)]` = `(rel_path, contents)`, already produced by `read_bundle` / `load_bundle_and_model`.
- Produces: `pub fn source_for<'a>(bundle: &'a [(String, String)], key: &str) -> Option<&'a str>` — returns the contents of the entry whose path basename without the `.md` extension equals `key`; `None` if no match. Task 2 calls this.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/waml-editor/src/load.rs`:

```rust
#[test]
fn source_for_matches_top_level_slug() {
    let bundle = vec![
        ("order.md".to_string(), "# Order\nbody".to_string()),
        ("customer.md".to_string(), "# Customer".to_string()),
    ];
    assert_eq!(source_for(&bundle, "order"), Some("# Order\nbody"));
}

#[test]
fn source_for_matches_nested_slug_by_basename() {
    let bundle = vec![("shop/order.md".to_string(), "# Order".to_string())];
    assert_eq!(source_for(&bundle, "order"), Some("# Order"));
}

#[test]
fn source_for_returns_none_when_absent() {
    let bundle = vec![("order.md".to_string(), "# Order".to_string())];
    assert_eq!(source_for(&bundle, "invoice"), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor source_for`
Expected: FAIL — `cannot find function source_for in this scope`.

- [ ] **Step 3: Implement the helper**

Add above the `#[cfg(test)]` module in `crates/waml-editor/src/load.rs`:

```rust
/// Return the raw markdown of the bundle file whose basename (minus `.md`)
/// equals `key`. `key` is a bare classifier slug (`order`); bundle paths may be
/// nested (`shop/order.md`), so match on the final path segment. `None` when no
/// file matches.
pub fn source_for<'a>(bundle: &'a [(String, String)], key: &str) -> Option<&'a str> {
    bundle.iter().find_map(|(path, contents)| {
        let base = path.rsplit('/').next().unwrap_or(path);
        let stem = base.strip_suffix(".md").unwrap_or(base);
        (stem == key).then_some(contents.as_str())
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor source_for`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/load.rs
git commit -m "feat(source-view): source_for bundle-by-slug lookup helper"
```

---

### Task 2: Render the markdown in the Source tab slot

Swap the opaque `SolidView` placeholder for a scrolling `Markdown` widget and feed it the subject's source in `sync_active_tab`.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` — the `source_view` slot in `live_design!` (currently `source_view := SolidView{ ... }`, around `app.rs:208`) and the `sync_active_tab` block that toggles the slot visible (around `app.rs:408-410`).

**Interfaces:**
- Consumes: `crate::load::source_for` (Task 1); `self.bundle: Vec<(String, String)>`; the active tab (`active.key`, `active.kind == TabKind::Source`).
- Produces: no new public API. The `source_view` slot now contains a child `md = Markdown{...}`, reachable via `self.ui.widget(cx, ids!(source_view, md)).as_markdown()`.

- [ ] **Step 1: Swap the slot widget in `live_design!`**

Replace the existing placeholder (around `app.rs:208`):

```rust
        source_view := SolidView{
            width: Fill
            height: Fill
            visible: false
            draw_bg.color: atlas.canvas_ground
        }
```

with a scrolling, opaque `View` wrapping a `Markdown` (mirror the Atlas scrollbar styling already used in `inspector_panel.rs:122`):

```rust
        // View Source tab body: renders the subject's raw markdown via the
        // upstream Markdown widget, scrolling vertically when it overflows. The
        // opaque `canvas_ground` bg preserves the occlusion contract (this slot
        // draws over the canvas whenever a Source tab is active).
        source_view := View{
            width: Fill
            height: Fill
            visible: false
            show_bg: true
            draw_bg.color: atlas.canvas_ground
            flow: Down
            scroll_bars: ScrollBars{ scroll_bar_y: ScrollBar{} }
            md = Markdown{
                width: Fill
                height: Fit
            }
        }
```

- [ ] **Step 2: Feed the source in `sync_active_tab`**

In `crates/waml-editor/src/app.rs`, at the block that already toggles the slot visible (around `app.rs:408`):

```rust
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        body.source_view(cx)
            .set_visible(cx, active.kind == TabKind::Source);
```

immediately after it, feed the markdown when the active tab is a Source tab:

```rust
        if active.kind == TabKind::Source {
            let md = crate::load::source_for(&self.bundle, &active.key)
                .map(str::to_string)
                .unwrap_or_else(|| format!("*No source for `{}`*", active.key));
            self.ui
                .widget(cx, ids!(source_view, md))
                .as_markdown()
                .set_text(cx, &md);
        }
```

- [ ] **Step 3: Build and run existing tests**

Run: `cargo build -p waml-editor` then `cargo test -p waml-editor`
Expected: build succeeds; all existing tests pass (no trait or tab-lifecycle change). If `as_markdown()` or the `Markdown` DSL name does not resolve, confirm `makepad_widgets::*` is in scope in the `live_design!` and that the fork rev exposes the widget (see Global Constraints).

- [ ] **Step 4: Clippy gate**

Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: no warnings (the implement-plan gate promotes `dead_code` to errors — the helper is now called, so it is live).

- [ ] **Step 5: Per-pid visual verification**

Launch the worktree's own build (see run-native), capture by its specific pid, and confirm:
- Right-click a classifier in the ProjectTree → "View Source" → a new **Source** tab opens showing rendered markdown (H1 title, `## Attributes` heading, bulleted attribute list, links styled).
- Right-click a node on the canvas → "View Source" → same rendered tab.
- A long document scrolls vertically inside the tab.
- The Source tab still fully occludes the canvas (opaque bg).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "feat(source-view): render subject markdown in the View Source tab"
```

---

## Self-Review

- **Spec coverage:** slot widget (§Approach.1) → Task 2 Step 1; feed text (§Approach.2) → Task 2 Step 2; key→source + raw-bundle choice (§Approach.3) → Task 1 + Task 2 Step 2; missing-key italic note → Task 2 Step 2 `unwrap_or_else`; occlusion contract → Task 2 Step 1 opaque bg; testing (unit + visual) → Task 1 tests + Task 2 Steps 3-5. No new triggers (non-goal) — honored, both paths untouched.
- **Placeholder scan:** none — every code step shows complete code.
- **Type consistency:** `source_for(&[(String, String)], &str) -> Option<&str>` defined in Task 1, called with `&self.bundle` + `&active.key` in Task 2; `.map(str::to_string)` bridges `&str`→owned for the `set_text(&str)` call.
