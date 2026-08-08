# Granular projection mask — design

Date: 2026-08-08

## Problem

The editor has one session-wide binary switch, `folder_projection::ViewMode`
(`Projected` / `Raw`). `Raw` pins the chain to `Chain::raw()`, bypassing
**every** declared `view:` stage at once.

That switch exists for exactly one reason, stated in `folder_projection.rs`:
reachability and diagnosis. A chain hid a row, or a chain misbehaves, and a
reader needs to see past it. It is presentational and never a permission
boundary.

All-or-nothing makes diagnosis imprecise. The question a reader actually has is
"*which* stage ate my row", and the answer today is "turn everything off and
see". There is also no way to say "run everything except this extension's
stages".

## Goal

Replace the binary switch with a **projection mask**: a set of disabled
middleware names, toggled per extension and per stage from a popup on the tree
panel's toolbar.

Session-only. Nothing persisted, no `.waml/settings.json` entry, no gate. Every
launch starts with an empty mask, so an author's declared `view:` is what a
reader sees unless they ask otherwise — the same rule `ViewMode` documents
today, carried forward.

## Non-goals

- **Search / tree filter.** Undecided, and likely to land as a prominent
  content search rather than a tree-local filter. A full-width filter field
  beneath the toolbar is purely additive and reworks no toolbar geometry, so
  deferring it incurs no layout debt.
- **True extension uninstall.** "Behave as if this extension isn't installed" —
  stages *and* icons *and* surfaces gone — belongs to the deferred profile
  system. It needs icon fallbacks, a story for already-open documents, and
  persistence. Masking here affects projection only.
- **Persistence.**
- **Developer mode.** Considered and dropped: with only two extensions
  shipping, gating the stage level hides almost nothing while adding a
  persisted setting.

## Key constraints discovered

These shape the design and are not optional.

### Disable cannot mean "absent from the registry"

`Chain::build` (`crates/waml/src/view/chain.rs:235-243`) treats an unknown
middleware name as a declaration-level failure: it returns `Chain::root_only`
— the **whole** chain collapses — plus an `UnknownViewMiddleware` diagnostic.

So unregistering a disabled name would collapse any folder declaring it all the
way back to raw, destroying the granularity this design exists to provide, and
would spray diagnostics that read as author errors. Disable must be a **skip**
inside build: drop the matched stage, keep every sibling, emit nothing.

The same applies to `hide`'s parameter check
(`crates/waml/src/view/chain.rs:221-227`), which returns `Chain::root_only` on
malformed `hide:` globs. Left ungated, a bad `hide:` would still collapse the
chain while `hide` is switched off.

### The registry has forgotten who owns what

`MiddlewareRegistry::from_extensions`
(`crates/waml/src/view/chain.rs:91-104`) deliberately flattens to one flat
`name -> factory` table so a duplicate name is a build error rather than
last-write-wins. Extension-level masking needs the owner back.

### `index` is the terminal stage, not a maskable one

`RootView` (`crates/waml/src/view/root.rs:1-7`) is "the terminal stage every
chain ends at… reached whenever a chain runs out of declared stages". Masking
`index` cannot remove the listing; the runner's terminal fallback lands on
`RootView` regardless. It is omitted from the popup.

Opening the markdown viewer over `index.md` is the separate `markdown`
*resolution* (`view: markdown`), handled inline in `Chain::build` and not a
registry stage — masking could not produce it.

### The universe is small today

`CoreExt` owns `index` and `hide`; `UmlExt` owns `uml`
(`crates/waml/src/extension.rs`). `markdown` and `member` are resolutions, not
stages. So the meaningful lists are extensions `{core, uml}` and maskable
stages `{hide, uml}`.

Both levels are built anyway: the mask mechanism is identical either way (an
extension toggle is derived as "all of its names"), so layering costs only the
nested UI, and the shape is right for when extensions multiply.

### The popup contract is pick-one-and-close

`PopupItem` (`crates/waml-editor/src/popup/base.rs:11`) carries
id/label/icon/danger/enabled and no checked state, and `PopupResult::Invoked`
closes the surface. A checklist needs toggle-and-stay, so the contract is
extended.

## Design

### Core — `waml` crate

**`waml::view::mask::ProjectionMask`** — a set of disabled middleware names,
with `is_masked(name)`. `Default` is empty, which is today's behavior exactly.
It lives in `waml`, not `waml-editor`, because the CLI and the vscode server
run the same chain path.

**`MiddlewareRegistry`** records ownership: `name -> (owner, factory)`, owner
from the existing `CoreExtension::name()`. The duplicate-name check is
unchanged. The registry exposes its owner grouping (extension name → its
middleware names) so the editor builds the layered popup from the registry
rather than a second hand-written extension list — the "two construction sites
that disagree" failure `folder_projection.rs:37-44` already warns about.

**`Chain::build`** takes the mask and:

1. computes ids via `ViewId::disambiguate` over the **declared** names first,
   unchanged, so surviving stages keep the ids they would have had unmasked —
   flipping the mask never silently renumbers owners;
2. skips a masked name silently — no stage pushed, no id pushed, no diagnostic;
3. gates `hide`'s parameter check on the mask;
4. leaves unknown-name behavior exactly as-is — whole-chain fallback plus
   `UnknownViewMiddleware`.

`markdown` / `member` resolutions are untouched.

**`ViewMode` is deleted.** Full raw becomes "every maskable name masked", so one
value describes what is running instead of a mode and a mask that can disagree.
A `RowId` owned by a now-masked stage falls to the existing unmatched-owner path
in `Chain::resolve` / `Chain::apply` — the same thing a `ViewMode` flip does
today.

### Editor — `waml-editor`

`App` holds the `ProjectionMask` in place of `ViewMode`. `chain_for` and
`project_rows` in `folder_projection.rs` take `&ProjectionMask`.

**Toolbar.** The tree panel gains a second thin row beneath its existing header.
The header stays the identity row (burger, title, collapse); the toolbar is
icon-only `IconButton`s in a left-aligned cluster, in order: projection button,
`ListCollapse`, `ListExpand`. Both list glyphs are already in the catalog.

Deliberately not the Visual Studio look — no split buttons with carets, no
ambiguous dropdown affordances.

**Projection glyph, three states**, replacing `view_mode_icon`'s two:

| Mask | Glyph |
|---|---|
| empty | `SquareLibrary` |
| partial | `SquareSplitHorizontal` (new) |
| every maskable name | `SquareCode` |

The glyph reports current state, not the action the button performs — the rule
`tree_panel.rs:899-901` already states.

`SquareSplitHorizontal` is a new catalog entry generated from the Lucide svg via
`scripts/gen-icon.py`. `SquareDashedTopSolid` was rejected: it is already the
`Interface` tree kind in this same panel (`tree_panel.rs:262`).

The svg is not vendored yet — `resources/icons/` holds only the handful of
glyphs generated so far — so `square-split-horizontal.svg` must be added there
first, following the existing Lucide-derived entries. Per-glyph SDF fit is a
known hazard: if the stroke clips or the glyph reads too small beside
`SquareLibrary` / `SquareCode`, nudge `A`/`B` for this glyph in the harness
rather than changing the shared fit.

**Popup.** The projection button opens `MenuPopup` in a new sticky mode. Rows:
one group per extension owning maskable stages, each with an extension-level
toggle, and its stage toggles nested beneath. Toggling an extension masks or
unmasks all of its names. `index` is omitted.

`PopupItem` gains `checked: Option<bool>` — `None` means a plain item, so every
existing item behaves identically. `MenuPopup` gains a sticky open where
`Invoked` reports the toggle without closing the surface.

**Collapse-all / expand-all** are two separate buttons, both always active. A
partially-expanded tree has no honest value for one toggling glyph to report.

## Testing

- `waml`: masking one stage keeps its siblings; a masked `hide` with malformed
  globs does **not** collapse the chain; an unknown name still does; ids are
  stable across mask flips; an empty mask reproduces pre-change output.
- `folder_projection`: an extension toggle masks exactly that extension's
  names; `index` never appears as maskable.
- `tree_panel`: the glyph reports all three states; toolbar buttons are live
  mounted children, following the existing
  `the_toggle_button_is_a_live_mounted_child` test.

Visual checks cannot be automated and are owed after implementation: toolbar
row layout and spacing, popup group/nesting legibility, and whether the three
glyphs read as one control's states.

## Known traps

- Adding a fixed-height row above the tree list touches the geometry that has
  previously blanked FileTree row labels (a lone fixed child filling a fixed
  parent). Verify row labels still draw after the toolbar lands.
- The new icon must respect the catalog's `enum == field == DSL == get == ALL
  == label` ordering and its count assertions.
- `PopupItem` is constructed by four popup surfaces; adding a field touches all
  of them.
