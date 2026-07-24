# Chrome Typography Scale — Design

## Problem

Editor chrome sets type ad-hoc. Font size is a bare literal at ~40 call sites
spanning six steps (10/11/12/13/14/16), and the font *family* is declared
inline per widget — some `draw_text` blocks hand-roll a `FontMember` pointing
at `IBMPlexSans-Regular.ttf`, others write `theme.font_regular{font_size: N}`.
There is no single place that owns typography, so the chrome reads slightly
inconsistent and can't be retuned globally. A partial scale already exists
(`atlas.size_eyebrow/caption/body/title`) but only three files use it.

The trigger: the ProjectTree header read as messy — an oversized 16pt title and
a tofu box (a `\u{2304}` text char IBM Plex Sans lacks). That's a symptom of the
larger gap.

## Goals

- One source of truth for chrome typography: a set of **semantic role tokens**,
  each bundling *family + size + weight + line-spacing*.
- Every chrome widget references `<role>` — **no inline `FontMember`, no bare
  `font_size` literal** in chrome.
- Tighten the ramp: weight and one accent family carry hierarchy, so fewer
  distinct sizes still read as a clear order.
- Fix the ProjectTree tofu (real SDF chevron) and its oversized title as the
  first consumers of the new scale.

## Non-goals / scope

**In scope (chrome):** app/caption bar, tree panel, inspector panel, doc tabs,
status bar, selection toolbar, diagram switcher, conflict badge, section
heading, all popups (menu/select/radial/conflict_list), select_box, shortcuts
overlay, ref_card, attr_row, action_link, recent_row, start_screen.

**Out of scope:**
- **Canvas diagram text** (`canvas.rs`) — zoom-scaled *content* rendering, its
  own mono/sans draw-vars keyed by weight+color. Not chrome.
- **`node_design_editor.rs`** — harness-only mock (not mounted live), hardcoded
  hex + sizes; left alone.
- No global ramp-shave this pass. If the chrome still reads big once live, we
  drop every step one notch as a follow-up (title 15 / body 11 / …). Deferred
  deliberately, not forgotten.

## The role set

Two families of discipline (matching how macOS text styles and Windows/Fluent
type ramps work — one system family, hierarchy via size+weight), plus **one
deliberate accent family** for the single most prominent title.

| Token | Family / weight | Size | Line | Used for |
|---|---|---|---|---|
| `text_title` | **Plex Sans Condensed SemiBold** (accent) | 16 | 1.1 | window/caption bar title; shortcuts-overlay title — the rare big moment |
| `text_heading` | Plex Sans SemiBold | 13 | 1.2 | panel titles, section headings, card titles, inspector element name |
| `text_body` | Plex Sans Regular | 12 | 1.2 | default UI text — values, toolbar, tree rows, switcher label |
| `text_label` | Plex Sans Medium | 11 | 1.2 | field labels, dim/secondary text, status bar, meta |
| `text_menu` | Plex Sans Regular | 10 | 1.2 | dense interactive popup/menu rows |
| `text_eyebrow` | Plex Sans SemiBold | 10 | 1.2 | UPPERCASE section eyebrows (RECENT / ALLOWED STEREOTYPES) |
| `text_mono` | Plex Mono Regular | 11 | 1.2 | IDs / counts / metrics in chrome |

Notes on the tightening:
- **14 collapses:** the few 14pt sites (inspector element name, select_box) go
  to `heading` 13.
- **Panel titles shrink 16 → 13** (`heading`). `text_title` 16 is reserved for
  the caption/window title so the accent font stays rare and prominent — this
  also resolves the "tree title too big" complaint.
- **Menus stay 10** via the dedicated `text_menu` — no bump (Windows treats
  menus as its smallest chrome text; user confirmed the density concern).
- **dim vs primary is a color concern** (`atlas.text` / `atlas.text_dim`), not a
  type role — so `label` covers both labels and captions.
- Weight now does real work: Medium/SemiBold cuts ship on disk
  (`IBMPlexSans-Medium.ttf`, `-SemiBold.ttf`, `IBMPlexSans_Condensed-SemiBold.ttf`).

## Where the tokens live

Type is **mode-independent** — families don't change between light and dark, so
the tokens must NOT be duplicated inside `atlas_light`/`atlas_dark` (the way
`size_*` currently is). They live once in a dedicated mode-independent module
alongside the theme, e.g. `mod.fonts` (final identifier chosen in the plan —
avoid `type`, a likely-reserved word). Each token is a full `TextStyle` with a
`FontFamily`/`FontMember` `crate_resource(...)` path into
`waml-editor/resources/fonts/`.

Widgets change from:
```
text_style: theme.font_regular{font_size: 11}
// or an inline FontMember{res: "...IBMPlexSans-Regular.ttf"} + font_size
```
to:
```
use mod.fonts
...
text_style: fonts.label
```

The existing `atlas.size_eyebrow/caption/body/title` tokens are **removed** once
their three consumers (action_link, recent_row, start_screen) migrate to roles.

## Migration mapping (principles + non-obvious calls)

Rule: map each site to the nearest role **by function**; where a site could go
two ways, **prefer the smaller/denser role** (honours the density goal); any
mapping that would *increase* a size gets flagged in the plan for confirmation.

Representative / non-obvious decisions:
- app caption model-name (16) → `text_title`; caption sub-heading (13) → `text_heading`.
- tree scope-title → `text_heading` (already at 13 from the in-flight edit).
- inspector panel title (16) + element name (14) → `text_heading` (13).
- doc-tab labels (10) → `text_menu` (compact nav; stays 10, no bump). The 18pt
  doc-tab `\u{d7}` close glyph is a **glyph metric, not type** — left as-is.
- popups + select_box row (10) → `text_menu`.
- attr_row / inspector labels / status bar (11) → `text_label`.
- shortcuts-overlay title (bold 16) → `text_title`; its key rows (13) → `text_heading`.

Exhaustive 1:1 line mapping is produced in the implementation plan.

## Folded-in fix (already in flight)

`tree_panel.rs` edits made before this design are kept and become the first
consumers:
- scope-title tofu: the `\u{2304}` text char → a real `Icon::ChevronsUpDown` SDF
  glyph (matches the type chip's Select affordance).
- title 16 → 13 (now `text_heading`); dim 12 → 11 (now `text_label`); label
  centering nudged for the smaller cut.

## Risks / watch-items

- **`type` keyword** — pick a safe module identifier (`fonts`), not `type`.
- **Weight-specific TTFs load** — Medium/SemiBold/Condensed-SemiBold must
  register; verify glyphs render (not fallback) at build.
- **Condensed metrics** — the accent cut may need its own `asc`/`desc` fudge
  (see the known makepad ascender/descender trim behavior); tune the caption
  title's vertical centering after the swap.
- **TextStyle-as-token referencing** — confirm makepad lets a widget point
  `text_style` at a whole named `TextStyle` from another module (the existing
  `theme.font_regular{...}` override proves the shape; verify bare reference).

## Testing / verification

- `cargo test --workspace` green.
- Full build (`pnpm build` / wasm+okf) green.
- **Grep gate:** zero bare `font_size:` literals and zero inline `FontMember`
  remain in chrome files (canvas + node_design_editor excluded).
- **Live visual** (per-pid screenshot, never kill-all — see the editor
  screenshot recipe): caption title reads in the condensed accent; panel titles
  at 13; menus still dense at 10; tree tofu gone.
