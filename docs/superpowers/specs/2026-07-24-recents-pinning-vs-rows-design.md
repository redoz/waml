# Recents: pinning + Visual Studio-tight rows — design

Date: 2026-07-24
Branch: `start-recents-5`
Status: design, awaiting review

## Problem

The start screen's recent-models list has two shortcomings:

1. **No pinning.** Every recent is transient MRU. A model you return to
   constantly falls off the bottom as newer projects push it down. Visual
   Studio's start page solves this with an on-hover pin toggle that keeps a
   project on the list and floats it to the top.
2. **Loose rows.** Our row pitch reads ~64px against VS's ~40px. The 7px accent
   marker, the `top/bottom: 5` row padding, and three full-line-box text runs
   make each row taller and airier than VS's tight `[icon] [title/path] [when]
   [pin]` anatomy.

## Goals

- On-hover pin button per row (VS-style). Pinning keeps the recent on the list
  regardless of the unpinned cap, and sorts it into a pinned block at the top.
- Tighten the row to VS anatomy: `[16px doc glyph] [title / path stacked tight]
  [when, right] [pin button, right]`.
- Size the list container to show exactly 5 rows; more than 5 scroll.

## Non-Goals

- No drag-reorder of pins (pins order by pin time, not manual rank).
- No new SDF glyph art — reuse `Icon::Package` (left doc glyph) and `Icon::Pin`
  (pin button); both already exist in the catalog.
- No change to how recents are recorded (`push_recent` on open stays as-is).
- No change to the empty-state ("No recent models") row.

## Locked Decisions

- **Row anatomy:** full VS anatomy (left glyph + tight text + when + pin).
- **Overflow:** the box always shows 5 rows. With 5+ pins, the pinned block
  fills all 5 visible slots (unpinned recents scroll below).
- **Cap:** pins are exempt from the unpinned cap. The `RECENTS_CAP` truncation
  only drops the oldest *unpinned* recents; pinned entries never get pruned by
  the cap.
- **Left glyph:** `Icon::Package` at 16px, accent-dim tint.
- **Pin button:** owns its own hit-rect + `MouseCursor::Hand` + a subtle wash
  distinct from the row hover. Clicking the pin toggles pin state and does NOT
  open the model (pin hit-test wins over the row's FingerUp).

## Design

### §1 `config.rs` — data model

Add pinned state to the persisted recent:

```rust
struct Recent {
    path: PathBuf,
    title: String,
    opened_at: u64,
    #[serde(default)]
    pinned_at: Option<u64>, // Some(unix) when pinned; None otherwise
}
```

`#[serde(default)]` keeps existing `editor.json` files forward-compatible — old
records deserialize with `pinned_at: None`.

New getter `pinned(&self) -> bool { self.pinned_at.is_some() }`.

New pure sort fn, applied wherever the list is read:

```rust
/// Pinned block first (oldest pin on top, so a fresh pin lands directly
/// below the last-pinned item), then unpinned MRU (newest first).
fn sort_recents(recents: &mut Vec<Recent>) {
    recents.sort_by(|a, b| match (a.pinned_at, b.pinned_at) {
        (Some(ap), Some(bp)) => ap.cmp(&bp),          // pinned: ascending pin time
        (Some(_), None) => Ordering::Less,            // pinned before unpinned
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.opened_at.cmp(&a.opened_at), // unpinned: MRU
    });
}
```

"Directly below the last pinned item" falls out of ascending `pinned_at`: the
newest pin has the largest timestamp, so it sorts last within the pinned block.

Cap becomes pin-exempt in `add_or_promote`: instead of a blanket
`truncate(RECENTS_CAP)`, retain all pinned entries and cap only the unpinned
tail. Sketch:

```rust
// after dedup + front-insert:
sort_recents(recents);
let mut kept_unpinned = 0;
recents.retain(|r| {
    if r.pinned() { return true; }
    kept_unpinned += 1;
    kept_unpinned <= RECENTS_CAP
});
```

New public API on the config store:

```rust
/// Set/clear the pin on the recent whose canonical path matches `path`.
/// Stamps `pinned_at = Some(now_unix())` on pin, `None` on unpin. Persists.
pub fn set_pinned(&mut self, path: &Path, pinned: bool);
```

`recents()` runs `prune_missing` then `sort_recents` before returning, so all
readers see the pinned-block-first ordering.

### §2 `recent_row.rs` — VS-tight anatomy

New child layout inside `RecentRowView`:

```
[ Package glyph 16px ]  [ title (Fill) .......... when ]   [ Pin button ]
                        [ path                          ]
```

Changes:

- Replace the 7×7 `marker` square with a 16px `Icon::Package` glyph drawn via
  `IconSet::draw`, accent-dim tint. (Left column, vertically centered by row
  `align`.)
- Row padding `top/bottom: 5.0` → `3.0` to tighten pitch toward VS. Keep the
  `textcol` `spacing: -2.0` already landed.
- Keep fonts: title `fonts.text_label`, path `fonts.text_menu`, when
  `fonts.text_label`.
- Add a trailing `pin` button child (its own View + hit-rect). Visibility
  states via a `pin_state` uniform on its draw_bg:
  - **unpinned + row not hovered:** hidden (alpha 0).
  - **unpinned + row hovered:** dim outline pin (low alpha).
  - **pinned:** lit accent pin (full), shown regardless of hover.
- Pin button self-tests its own area in `handle_event` and emits
  `RecentRowViewAction::TogglePin` on FingerUp, claiming the hit before the row
  body's `Clicked` (order the pin's `hits` check first / bail row click when the
  pin consumed it).
- New action variant + reader + setters:
  - `RecentRowViewAction::{ Clicked, TogglePin }`.
  - `set_pinned(&mut self, cx, bool)` drives the pin glyph/state uniform.
  - `toggle_pin(&self, actions) -> bool` reader (mirror of `clicked`).
  - Same on `RecentRowViewRef`.
- New associated const `RecentRowView::ROW_HEIGHT: f64` — the measured row pitch
  (glyph 16 + 2×3 padding, floored to the text stack height) — so the container
  can size to exactly 5 rows without a magic number in `start_screen.rs`.

### §3 `start_screen.rs` — container + row data

- Drop the fixed `height: 320.0` on `list_frame`; set it in `draw_walk` to
  `5.0 * RecentRowView::ROW_HEIGHT + 2.0 * 5.0` (5 rows + the existing Inset-5
  top/bottom padding). FlatList scrolls beyond 5.
- `RecentRow` render-copy gains `pinned: bool`, pushed to the row via
  `set_pinned` in the draw loop.
- Route the pin toggle: read `row.toggle_pin(actions)` alongside
  `row.clicked(actions)` in `handle_actions`, map item_id → index via the
  existing `row_index_for`, emit `StartScreenAction::TogglePin(i)`.

### §4 action routing — `app.rs`

- Remove the uncommitted `.take(5)` in `show_start_screen` (the container now
  bounds the visible count; all recents feed the FlatList so pins beyond 5
  remain reachable by scroll).
- New `StartScreenAction::TogglePin(usize)`.
- App handles `TogglePin(i)` by looking up the recent's path, calling
  `config::set_pinned(path, !currently_pinned)`, then reloading
  `self.start_recents` from `config.recents()` and redrawing.

## Data Flow

Open model → `push_recent` (unchanged, `pinned_at: None`) → `add_or_promote`
(pin-exempt cap) → persist. Start screen read → `recents()` → `prune_missing` →
`sort_recents` (pinned block first) → rows. Pin click → `TogglePin(i)` →
`set_pinned` → persist → reload → re-sorted rows redraw.

## Testing

Pure `config.rs` unit tests (no filesystem):

- `sort_recents`: mixed pinned/unpinned orders pinned-block-first, pins
  ascending by `pinned_at`, unpinned MRU.
- new pin lands directly below the last existing pin.
- pin-exempt cap: with `RECENTS_CAP` unpinned + N pinned, all pinned survive and
  exactly `RECENTS_CAP` unpinned are kept.
- `set_pinned` round-trip through the hand-rolled TempDir: pin, reload, assert
  `pinned()`; unpin, reload, assert cleared.
- serde forward-compat: an `editor.json` payload with no `pinned_at` field
  deserializes to `None`.

Shape-gate tests (headless, no VM boot):

- `RecentRowView::ROW_HEIGHT` present and used for `list_frame` height (grep the
  container assignment references the const, not a literal).
- `StartScreenAction::TogglePin` routed from the row `toggle_pin` reader.

## Risks

- **ROW_HEIGHT drift.** If the row's real drawn pitch diverges from the const,
  the box shows 4.5 or 5.5 rows. Mitigate: derive the const from the same
  padding/glyph numbers the DSL uses, verify the 5-row fit by pid-specific
  screenshot after implementing.
- **Pin vs row event ordering.** If the row body claims FingerUp first, a pin
  click also opens the model. Mitigate: test the pin's `hits` before the row's,
  bail the row `Clicked` when the pin consumed the press; verify interactively.
- **editor.json forward-compat.** Covered by `#[serde(default)]` + the
  no-`pinned_at` deserialize test; old configs must not error on load.
