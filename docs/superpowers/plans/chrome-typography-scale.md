# Chrome Typography Scale — Implementation Plan

Implements `docs/superpowers/specs/2026-07-24-chrome-typography-scale-design.md`.

## Summary

Introduce a mode-independent `mod.fonts` module holding **7 semantic `TextStyle`
role tokens** (family + size + weight + line), then migrate **every** chrome
`font_size` literal and inline `FontMember` to reference a role. Fold in the
`tree_panel` tofu/oversize fix as the first real consumer. Remove the partial
`atlas.size_*` scale once its three consumers migrate.

The module lands FIRST (foundation), then files migrate in small independently
committable batches. `atlas.size_*` is deleted only in the unit where its last
consumer migrates. A final unit greps the chrome for zero residual bare
`font_size:` / inline `FontMember` (documented exceptions aside).

**Per-unit gate (every unit must pass on its own):**
```
cargo test --workspace && pnpm -r test && pnpm lint && pnpm build
```
`pnpm build` (wasm+okf) matters because the DSL/`script_mod` registration is only
fully exercised by a real build; a green `cargo test` alone will not catch a
mis-ordered `script_mod(vm)` registration or an unresolved `mod.fonts` reference.

## The 7 role tokens (authoritative)

| Token | TTF (`self:resources/fonts/...`) | size | line |
|---|---|---|---|
| `text_title` | `IBM_Plex_Sans/IBMPlexSans_Condensed-SemiBold.ttf` | 16 | 1.1 |
| `text_heading` | `IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf` | 13 | 1.2 |
| `text_body` | `IBM_Plex_Sans/IBMPlexSans-Regular.ttf` | 12 | 1.2 |
| `text_label` | `IBM_Plex_Sans/IBMPlexSans-Medium.ttf` | 11 | 1.2 |
| `text_menu` | `IBM_Plex_Sans/IBMPlexSans-Regular.ttf` | 10 | 1.2 |
| `text_eyebrow` | `IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf` | 10 | 1.2 |
| `text_mono` | `IBM_Plex_Mono/IBMPlexMono-Regular.ttf` | 11 | 1.2 |

All five TTFs are confirmed present on disk. Baseline trim for every token:
`asc: -0.1 desc: 0.0` (matches the existing inline `FontMember`s), EXCEPT
`text_title` (Condensed cut) which is tuned in Units 2–3 for vertical centering.

## Module identifier & wiring notes (read before Unit 1)

- Identifier is **`fonts`** (spec's suggested name; `type` is avoided as a likely
  reserved word).
- New file `crates/waml-editor/src/fonts.rs`, declared `mod fonts;` in
  `main.rs` (alphabetical slot: between `doc_view` and `fps_meter`). There is no
  `lib.rs`; all modules live under `main.rs`.
- `fonts.rs` mirrors `theme_atlas.rs`'s shape: a single `script_mod! { ... }`
  block that assigns `mod.fonts.text_title = TextStyle{ ... }` (× 7). Because the
  tokens are `TextStyle`/`FontFamily`/`FontMember`/`crate_resource` values (unlike
  `theme_atlas`'s scalar colors), the block MUST import the same script
  namespaces an existing inline-`FontMember` widget imports — copy the `use
  mod.prelude...` / `use mod.text.*` prelude lines from `statusbar.rs`'s
  `script_mod!` block so those types resolve. The Unit 1 build gate proves the
  import set.
- **Registration order (critical — the dead-token trap):** a module used as a
  DSL dependency must have its `script_mod(vm)` run BEFORE any consumer resolves
  it (cf. the IconButton child-order gotcha, and how `theme_atlas` registers at
  the very top of `App::script_mod`). Add `crate::fonts::script_mod(vm);` in
  `app.rs` `App::script_mod` immediately AFTER the `theme_atlas` block (the
  `mod.atlas` repoint, ~line 1776) and BEFORE `crate::icons::script_mod(vm)` and
  all widget registrations. If it registers after a consumer, the tokens resolve
  to nothing and the text silently vanishes/falls back.
- **Consumption pattern:** `atlas` is NOT auto-folded into the prelude — every
  widget writes `use mod.atlas` explicitly (the prelude only folds the fork's own
  `theme`). Mirror that: each migrated consumer adds `use mod.fonts` to its
  `script_mod!` block, then references a token bare:
  `text_style: fonts.text_body`. (The existing `theme.font_regular{...}` override
  proves TextStyle-as-token referencing; Unit 1 proves the *bare* reference.)

## Exhaustive site → role mapping

Line numbers are origin/main state (grep fresh in the worktree — line numbers
drift as edits land). Sites are grouped by their owning unit below.

**Size-increase flags (need live per-pid visual confirmation; see spec rule):**
1. `app.rs` caption `model_name` 13 → 16 (`text_title`). Deliberate: spec reserves
   `text_title` for the caption/window title as the "rare big moment."
2. `recent_row.rs` `when` + `path` 10 → 11 (`text_label`). Spec files timestamps/
   paths under "meta" = `text_label`; the +1 is the ramp-consolidation intent.

**Correction to the spec's by-size migration line:** `attr_row.rs` rows are
**monospace** (`IBMPlexMono-Regular`), so they map to **`text_mono`**, NOT
`text_label` (the spec grouped them by size 11; family wins — they are attribute
signatures and must stay mono for column alignment).

## Out of scope — do NOT touch

`canvas.rs`, `node_design_editor.rs`, `src/bin/*.rs` (all harnesses), and
`card/mod.rs` (taffy diagram-content measurement, native-only, feeds `canvas.rs`
— not chrome). These retain their own `font_size`/`FontMember` and are excluded
from the final grep gate.

---

### Task 1 — Foundation: `mod.fonts` module, wiring, and one proof consumer

**Context.** Everything downstream references `mod.fonts`; it must land first,
registered in the correct order, with the bare-token reference proven end-to-end.
Migrating one trivial single-token consumer in the same unit both proves the
reference shape and keeps the module from being unreferenced.

**Files.**
- Create `crates/waml-editor/src/fonts.rs`: one `script_mod!` block defining all
  7 tokens per the table above (import prelude/text namespaces as in
  `statusbar.rs`; baseline `asc: -0.1 desc: 0.0`, `text_title` line 1.1 and the
  rest 1.2).
- `crates/waml-editor/src/main.rs`: add `mod fonts;` (alphabetical slot after
  `mod doc_view;`).
- `crates/waml-editor/src/app.rs` (`App::script_mod`): add
  `crate::fonts::script_mod(vm);` right after the `theme_atlas` repoint block and
  before `crate::icons::script_mod(vm);`.
- `crates/waml-editor/src/statusbar.rs` (proof consumer): add `use mod.fonts`;
  replace the `draw_text` inline `TextStyle{ font_size: 11 ... Regular ... }`
  (~lines 25–31) with `text_style: fonts.text_label`.

**Verification.**
- Full gate green (build proves registration order + bare-reference shape).
- Grep `crates/waml-editor/src/statusbar.rs` for `font_size` / `FontMember`:
  zero hits.
- Live (optional this unit): status bar text still renders (Medium 11), not tofu.

---

### Task 2 — `tree_panel.rs` fold-in fix (tofu → SDF chevron, title/dim, rows)

**Context.** The spec's trigger case. On origin/main this file is still the
ORIGINAL (title 16, dim 12, and a `\u{2304}` tofu char in the scope-title) — the
earlier hand-edits were never pushed, so re-specify them here. The type-chip
chevron already draws `Icon::ChevronsUpDown` via `self.icons.draw(...)` in
`draw_walk` (~line 779) — mirror that for the scope-title.

**Files.** `crates/waml-editor/src/tree_panel.rs` (add `use mod.fonts`):
- `draw_title` (`TextStyle{ font_size: 16 ... Regular ... }`, ~139–148) →
  `fonts.text_heading`.
- `draw_dim` (`TextStyle{ font_size: 12 ... Regular ... }`, ~149–158) →
  `fonts.text_label`.
- `file_node` `draw_text` (`theme.font_regular{font_size: 10}`, ~207) →
  `fonts.text_menu`.
- `folder_node` `draw_text` (`theme.font_regular{font_size: 10}`, ~231) →
  `fonts.text_menu`.
- Scope-title tofu (`draw_walk`, ~677): drop the `\u{2304}` from
  `format!("{title} \u{2304}")` (draw the bare `title`), and draw an
  `Icon::ChevronsUpDown` SDF glyph via `self.icons.draw(cx, Icon::ChevronsUpDown,
  <rect>, dim)` immediately after the title label (same idiom as the chip chevron
  at ~779). Recompute `title_rect` width to include the glyph so the click
  hit-test still covers the whole trigger.
- Vertical centering: with the title dropping 16→13 (SemiBold cut), re-tune the
  `title_pos` y-offset (currently `cy - 8.0`) so the smaller label seats on the
  title-row centerline.

**Verification.**
- Full gate green.
- Grep `tree_panel.rs`: zero `font_size:` / inline `FontMember`; zero `\u{2304}`.
- Live per-pid screenshot: scope title reads at heading-13; a real up/down
  chevron glyph (no tofu box); dim/search text at label-11.

---

### Task 3 — `app.rs` caption bar → `text_title`

**Context.** The caption/window title is `text_title`'s reserved home (the rare
Condensed accent moment). On origin/main the caption cluster is: logo · `/`
separator (16) · `model_name` (13 Medium). Both text elements move to
`text_title`. The `model_name` 13→16 is FLAG #1 (deliberate). `model_name`
currently carries a custom `asc: 0.1 desc: 0.15` trim for caption centering — the
Condensed cut needs its own trim, so tune `text_title`'s `asc`/`desc` here and
re-verify (this same token also drives the shortcuts title in Task 9).

**Files.** `crates/waml-editor/src/app.rs` (startup `script_mod!` block already
has `use mod.atlas`; add `use mod.fonts`):
- `sep` `/` label (`theme.font_regular{font_size: 16}`, ~89) → `fonts.text_title`.
- `model_name` label (`TextStyle{ font_size: 13 ... Medium ... asc: 0.1 desc:
  0.15 }`, ~113–116) → `fonts.text_title`.
- If the Condensed cut sits high/low in the 32px caption band, adjust
  `text_title`'s `asc`/`desc` in `fonts.rs` and re-verify both caption + shortcuts.

**Verification.**
- Full gate green.
- Grep `app.rs`: zero `font_size:` / inline `FontMember`.
- Live: caption title reads in the Condensed SemiBold accent, vertically centered
  in the caption bar; confirm the 13→16 growth reads as intended (FLAG #1).

---

### Task 4 — `inspector_panel.rs` (8 sites)

**Context.** Panel title shrinks 16→13; the 12/13/14 body sites consolidate. All
8 draw fields are inline `TextStyle`+`FontMember`.

**Files.** `crates/waml-editor/src/inspector_panel.rs` (add `use mod.fonts`):
- `kind` (11 Medium accent, ~144) → `fonts.text_label` (exact family/size match).
- `stereo` (11 Regular dim, ~158) → `fonts.text_label` (weight Regular→Medium).
- `desc` (12 Medium, ~245) → `fonts.text_body` (weight Medium→Regular).
- `draw_title` (16 Regular, ~259) → `fonts.text_heading`.
- `draw_label` (12 Regular dim, ~269) → `fonts.text_label`.
- `draw_dim` (12 Regular dim, ~279) → `fonts.text_label`.
- `draw_name` (13 Regular, ~305) → `fonts.text_heading` (card name line).
- `draw_glyph` (14 accent direction glyph, ~317) → `fonts.text_heading` (14→13
  collapse). Watch: this is a glyph drawn as text; verify the direction arrow
  still renders correctly under the SemiBold cut — if it degrades, treat it as a
  glyph-metric exception (keep its own `TextStyle`) and note it in the Task 11
  gate exclusions.

**Verification.**
- Full gate green.
- Grep `inspector_panel.rs`: zero residual (or only the `draw_glyph` exception if
  taken).
- Live: inspector header/name at heading-13; kind/stereo/labels at label-11;
  description body at body-12.

---

### Task 5 — `select_box.rs` + `doc_tabs.rs` (nav chrome)

**Context.** The inspector element-name picker (`select_box`) and the doc-tab
strip. `select_box` `draw_label` (14 bold) IS the "inspector element name (14)"
the spec collapses to heading-13. Doc-tab labels stay 10 (`text_menu`).

**Doc-tab state-font decision (DECIDED — preserve the device).** The four
tab-label draws share size 10 and differ only by weight/style to signal STATE
(persisted=Regular, active=Bold, preview=Italic, preview_active=SemiBoldItalic)
— the deliberate Zed-style provisional/selected device. The 7-role set has no
italic/bold-menu variant, so the three state variants cannot be expressed as a
role without destroying an existing feature. **Decision:** migrate ONLY the
resting `draw_text_persisted` to `text_menu`; KEEP `draw_text_active` /
`draw_text_preview` / `draw_text_preview_active` as documented STATE-font
exceptions (parallel to the `draw_close` 18pt glyph metric, which the spec
already excludes) and list them in the Task 11 gate exclusions. Do NOT flatten
the state variants — that would remove the provisional device, out of scope for
a typography-normalization pass.

**Files.**
- `crates/waml-editor/src/select_box.rs` (add `use mod.fonts`):
  - `draw_badge_text` (`theme.font_regular{font_size: 10}`, ~58) →
    `fonts.text_menu`.
  - `draw_label` (`theme.font_bold{font_size: 14}`, ~66) → `fonts.text_heading`.
- `crates/waml-editor/src/doc_tabs.rs` (add `use mod.fonts`):
  - `draw_text_persisted` (Regular 10, ~89) → `fonts.text_menu`.
  - `draw_text_active` (Bold 10) / `draw_text_preview` (Italic 10) /
    `draw_text_preview_active` (SemiBoldItalic 10): keep as documented state
    exceptions.
  - `draw_close` (18, ~131): LEAVE (glyph metric, per spec).

**Verification.**
- Full gate green.
- Grep `select_box.rs`: zero residual. Grep `doc_tabs.rs`: only the documented
  state/close exceptions remain.
- Live: element name at heading-13; tab labels dense at 10; active/preview state
  reads (bold / italic) intact; close glyph unchanged.

---

### Task 6 — Popups → `text_menu`

**Context.** Dense interactive menu/select rows, all `theme.font_regular{
font_size: 10 line_spacing: 1.2 }`.

**Files** (each add `use mod.fonts`, each row `text_style` → `fonts.text_menu`):
- `crates/waml-editor/src/popup/menu.rs` (~333).
- `crates/waml-editor/src/popup/select.rs` (~96 and ~127 — both rows).
- `crates/waml-editor/src/popup/radial.rs` (~357).
- `crates/waml-editor/src/popup/conflict_list.rs` (~154).

**Verification.**
- Full gate green.
- Grep the four popup files: zero `font_size:` / inline `FontMember`.
- Live: menu / select flyout / radial / conflict-list rows unchanged at 10.

---

### Task 7 — Inspector-content widgets: `section_heading`, `attr_row`, `ref_card`

**Context.** Eyebrow headings, monospace attribute rows, reference cards.

**Files.**
- `crates/waml-editor/src/section_heading.rs` (add `use mod.fonts`): `label`
  (SemiBold 10 uppercase dim, ~26) → `fonts.text_eyebrow` (exact match).
- `crates/waml-editor/src/attr_row.rs` (add `use mod.fonts`): all five labels
  (`vis`, `name`, `colon`, `ty`, `mult` — Mono Regular 11, ~28/41/54/67/80) →
  `fonts.text_mono` (see the spec correction — these are monospace signatures).
- `crates/waml-editor/src/ref_card.rs` (add `use mod.fonts`): `name`
  (Regular 13, ~72) → `fonts.text_heading`; `meta` (Regular 11 dim, ~85) →
  `fonts.text_label`.

**Verification.**
- Full gate green.
- Grep the three files: zero residual.
- Live: section eyebrows uppercase SemiBold-10; attribute rows stay mono &
  column-aligned; ref-card name at heading-13, meta at label-11.

---

### Task 8 — Small bars: `conflict_badge`, `diagram_switcher`, `selection_toolbar`

**Context.** Three small HUD bars, all inline `TextStyle`+`FontMember`.

**Files.**
- `crates/waml-editor/src/conflict_badge.rs` (add `use mod.fonts`): `label`
  (SemiBold 12 white count, ~40) → `fonts.text_body` (weight SemiBold→Regular; the
  danger-colored badge bg carries the emphasis — confirm on live it still reads).
- `crates/waml-editor/src/diagram_switcher.rs` (add `use mod.fonts`):
  `draw_label` (Regular 12, ~42) → `fonts.text_body`; `draw_caret`
  (Regular 11 dim ⌄, ~52) → `fonts.text_label`.
- `crates/waml-editor/src/selection_toolbar.rs` (add `use mod.fonts`):
  `draw_label` (Regular 12 dim, ~30) → `fonts.text_label` (12→11, secondary);
  `draw_action` (Regular 12, ~40) → `fonts.text_body`.

**Verification.**
- Full gate green.
- Grep the three files: zero residual.
- Live: conflict badge count legible; switcher label at body-12 + caret at
  label-11; selection toolbar label/action hierarchy holds.

---

### Task 9 — `shortcuts_overlay.rs`

**Context.** The overlay title is the second (only other) `text_title` home; its
key rows collapse to heading-13, descriptions to body-12 (denser than the key).

**Files.** `crates/waml-editor/src/shortcuts_overlay.rs` (add `use mod.fonts`):
- `draw_title` (`theme.font_bold{font_size: 16}`, ~38) → `fonts.text_title`.
- `draw_key` (Regular 13, ~43) → `fonts.text_heading`.
- `draw_desc` (Regular 13 dim, ~53) → `fonts.text_body` (keeps the description
  lighter/denser than the SemiBold key; deviates from a strict "key rows → heading"
  read — note for live confirmation).

**Verification.**
- Full gate green.
- Grep `shortcuts_overlay.rs`: zero residual.
- Live: overlay title in the Condensed accent (re-check the Task-3 `text_title`
  trim holds in this layout too); key/desc hierarchy reads.

---

### Task 10 — Start-screen trio + remove `atlas.size_*`

**Context.** The three remaining `atlas.size_*` consumers. Once they migrate, the
partial scale is dead (`size_title` is already unused) and gets deleted from BOTH
`atlas_light` and `atlas_dark`. `start_screen.rs`'s subtitle baseline-seating
reads `font_size` from the resolved `TextStyle` at runtime, so it keeps working
after the swap — but line-spacing shifts 1.0→1.2; verify the subtitle seats.

**Files.**
- `crates/waml-editor/src/action_link.rs` (has `use mod.text.*`; add
  `use mod.fonts`): `label` (`theme.font_regular{font_size: atlas.size_caption}`,
  ~93) → `fonts.text_label`.
- `crates/waml-editor/src/recent_row.rs` (add `use mod.fonts`): `title`
  (`atlas.size_body`, ~88) → `fonts.text_body`; `when` (`atlas.size_eyebrow`, ~97)
  → `fonts.text_label` (FLAG #2, 10→11); `path` (`atlas.size_eyebrow`, ~106) →
  `fonts.text_label` (FLAG #2, 10→11).
- `crates/waml-editor/src/start_screen.rs` (add `use mod.fonts`): `sub`
  (`atlas.size_caption`, ~129) → `fonts.text_label`; `recent_eyebrow` "RECENT"
  (`theme.font_bold{font_size: atlas.size_eyebrow}`, ~171) → `fonts.text_eyebrow`;
  `start_eyebrow` "START" (~230) → `fonts.text_eyebrow`.
- `crates/waml-editor/src/theme_atlas.rs`: delete `size_eyebrow`, `size_caption`,
  `size_body`, `size_title` from BOTH `atlas_light` and `atlas_dark` (and the
  now-stale type-scale comment block in each).

**Verification.**
- Full gate green (proves no remaining `atlas.size_*` reference anywhere).
- Grep the three widgets: zero `font_size:` / inline `FontMember` / `atlas.size_`.
- Grep whole crate for `atlas.size_`: zero hits.
- Live: start-screen subtitle seats correctly; RECENT/START eyebrows at
  eyebrow-10; recent rows title body-12, meta label-11 (confirm FLAG #2 reads).

---

### Task 11 — Final verification: grep gate + live pass

**Context.** Lock in "no ad-hoc chrome type." Prove zero bare `font_size:` and
zero inline `FontMember` remain in chrome files, save the documented exceptions.

**Documented exceptions (the ONLY allowed residue in chrome):**
- `doc_tabs.rs`: `draw_close` (18pt glyph metric), and — if the Task-5 recommended
  path was taken — `draw_text_active` / `draw_text_preview` /
  `draw_text_preview_active` (state fonts).
- `inspector_panel.rs`: `draw_glyph`, only if Task 4 kept it as a glyph-metric
  exception.

**Excluded files (not chrome):** `canvas.rs`, `node_design_editor.rs`,
`src/bin/*.rs`, `card/mod.rs`.

**Files.** No source migration; add a verification step (and optionally commit a
small `scripts/` grep check or a `#[test]` that scans the chrome sources) that
runs:
```
rg -n 'font_size:|FontMember' crates/waml-editor/src \
  --glob '!**/canvas.rs' --glob '!**/node_design_editor.rs' \
  --glob '!**/bin/**' --glob '!**/card/**'
```
The output must contain ONLY the documented-exception lines above (empty modulo
those). Any other hit fails the gate.

**Verification.**
- Full gate green.
- The grep above returns only documented exceptions.
- `rg -n 'atlas.size_' crates/waml-editor/src` → zero.
- Live per-pid screenshot pass (capture by specific pid; never kill-all — see the
  editor screenshot recipe): caption + shortcuts titles in the Condensed accent;
  panel titles at 13; menus/tabs dense at 10; tree tofu gone; both size-increase
  flags (caption model-name 16, recent meta 11) read as intended.
