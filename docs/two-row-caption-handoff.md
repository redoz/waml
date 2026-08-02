# Two-row caption bar — review handoff

Branch `title-tab-two-row` (worktree only, **not merged, not pushed**).
Rebased on `main`. Run it:

```
./run.ps1                             # mini fixture
```

## What changed

A taller **66px** caption (was 44px single row), Zed-inspired:

- **Big logo** (70×40) pinned left, spanning **both rows**.
- **Top row** — burger + the open model's name (12px Medium heading).
- **Bottom row** — the doc-tab strip in its own dedicated band.
- Min/max/close hug the top edge; the tab-strip rule is softened
  (`frame_hi`→`frame_lo`).

Files: `crates/waml-editor/src/app.rs` (caption DSL + burger anchor),
`popup/menu.rs` (`CAPTION_H` 44→66), `doc_tabs.rs` (`TOP_MARGIN` 14→8,
softer `draw_edge`).

## Seating note (for future tweaks)

The heading sits via the **`align y:0.5` + `asc:0.1 desc:0.15` trim**
recipe (same as the old single-row name). In this centring turtle,
`margin`/`padding` on the label are **absorbed and do nothing** — the
glyph's vertical seat comes only from the asc/desc trim. See
`makepad-font-asc-desc-trim`.

## Still to eyeball / decide

- **Interactivity click-test** — burger drop-down (now anchored off the
  caption bottom from the title row) and tab hover/press under the new
  nesting. Drag-query re-answers `Client` over the burger/logo/tabs, but
  I could not click-test headlessly.
- **Tab restyle** — the "make tabs cleaner" idea is untouched; the
  strip just moved to its own band. Open for a Zed-style pass.
