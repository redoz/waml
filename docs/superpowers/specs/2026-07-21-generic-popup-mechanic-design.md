# Generic popup mechanic — design

**Date:** 2026-07-21
**Target:** `waml-editor` (Makepad), immediate-mode
**Supersedes runtime of:** `2026-07-19-radial-command-menu-design.md`, `2026-07-19-logo-radial-menu-design.md` (their surfaces survive; their ad-hoc App routing is unified here)

## Summary

One dependable popup service for the editor. Any caller says "show this surface
as a popup here"; opening destroys the current active popup; Esc, window blur,
app-deactivate, any system action, or a click anywhere the popup didn't claim
destroys whatever is active. A single active popup, app-wide, owning **both**
kinds behind a common abstraction — the linear card (logo / burger / dropdown /
context) **and** the radial wedge menu.

The service is deliberately narrow: it is a **dismiss authority over one active
ephemeral surface**. It knows only how to register the active surface and how to
tell it to go away. It knows nothing about the surface's geometry, its items,
what an item means, how a selection routes back to whoever opened it, or where
the pixels land (in-window vs a separate compositor window). Those all belong to
the surface and the caller, never to the service.

## Motivation

Today two near-identical state machines (`radial::RadialCore`, `app_menu::
AppMenuCore`) and their two draw widgets are wired into `App` by hand: two
`is_open()`-gated `handle()` calls, two outcome matches, a `MenuOwner` enum plus
a `set_app_menu_owner` glow seam, a single manual reset point, and a
`WindowDragQuery` client-ize-while-open hook. Every new popup caller (the
inspector element-picker is the next one, and it currently draws its list
*inline*, growing the panel frame) would bolt more of the same onto `App`. We
want one seam that guarantees single-active + universal dismiss so callers never
re-implement any of it.

## Scope

**In:** the generic popup service, a content-blind **Presenter** (the backing
surface that owns where pixels land), the two surface kinds ported onto it, and
the App-side collapse. **Out (for now):** nothing pending — the previously
deferred feature (the Presenter) is now folded in; see [Presenter](#presenter).

## Naming (WinRT-rooted)

WinRT models transient surfaces well, so we borrow its vocabulary. Its base
primitive under `FlyoutBase` is literally `Popup`; a `PopupRoot` per `XamlRoot`
hosts the one active popup and runs light-dismiss. We take that pair and skip the
Microsoft-only "flyout" dialect.

- **`PopupRoot`** — the authority / the whole "system": single active slot +
  light-dismiss logic. (WinRT `PopupRoot`.)
- **`Popup`** — the trait every surface kind implements: a showable,
  light-dismissable surface. (WinRT `Popup` / `FlyoutBase`.)
- **`MenuPopup`** — the linear card surface (WinRT `MenuFlyout`).
- **`RadialPopup`** — the wedge surface. WinRT has no radial flyout (its
  `RadialController` is unrelated hardware chrome), so this is a consistent
  coinage, not a real WinRT type.
- **`Tag: LiveId`** — the opaque caller token that rides on a popup and comes
  back on close (WinRT `FrameworkElement.Tag`, an opaque user object).
- **light dismiss** — the kill triggers (WinRT `IsLightDismissEnabled` /
  `LightDismissOverlayMode`).
- **`Presenter`** — the content-blind backing surface: owns *where pixels land*
  and the event-space they arrive in. Platform decides the backing (Windows →
  DComp compositor window; web + all else → in-app overlay layer). No direct
  WinRT analogue — WinRT's `Popup` fuses content + compositing; we split them.
  (WinRT's `ShouldConstrainToRootBounds` per-`Popup` bool is what we *replace*:
  backing is a platform cfg, not a per-surface flag.)
- **overlay** — reserved for its real meaning: the in-app overlay layer the
  Presenter draws into on non-Windows (and the dismiss scrim `PopupRoot` may draw
  behind a popup). NOT the surface (that clash is why we chose `Popup`), and it
  already names makepad's `flow_overlay` draw layer in this crate.

## What WinRT validated

Grounded in the WinUI/UWP docs. The pieces we steal:

1. **Host owns show/place/dismiss; content owns routing — nothing crosses.**
   `Popup`/`FlyoutBase` never inspect `Command`/`CommandParameter`. Strong
   confirmation that our authority must be blind to what an item means.
2. **Opaque parameter carried on the surface, not brokered by the host.**
   WinRT's `CommandParameter: object` is exactly our opaque `Tag` — the standard
   shape, not an exotic one.
3. **Compositing is orthogonal to what the content is.**
   WinRT's `ShouldConstrainToRootBounds` lives on `Popup`, independent of whether
   the content is a menu — validating that "where pixels land" is a separate axis
   from "what's drawn." We take that orthogonality further: it is not even a
   per-surface flag, it is a **platform-cfg backing** owned by a `Presenter`, so
   content is fully blind to compositing (see [Presenter](#presenter)).

What we deliberately do **not** copy:

- **WinRT does not actually enforce single-active** — app code manually closes
  prior popups (`GetOpenPopups().ForEach(close)`). We make single-active a *hard
  guarantee* of `PopupRoot`. Strictly better than the platform.
- **No awaited host-level result** (`ContentDialog.ShowAsync` is the only such
  case, and only because it is a fixed-cardinality modal). Arbitrary menus route
  per-item; we do the fire-and-filter equivalent (below). No async runtime.
- **`RadialController`** — no reusable "one host, two geometries" precedent; our
  linear-vs-radial unification is our own call.

## Why not async

Considered modelling `popup.show(items).await`. Rejected on substrate and
semantics:

- The makepad fork has **no `Future` executor** — only `Cx::spawn(url)` (HTTP)
  and `spawn_thread` (OS thread). No `Waker`, no task-poll queue, no `block_on`.
  Nothing can resume a Rust task from inside `handle_event`. `await` would mean
  *building* an executor + oneshot + Waker/re-poll subsystem.
- Even built, the woken continuation **cannot hold `&mut Cx` across the await**
  (Cx is threaded by-reference through the sync loop, not owned by tasks). After
  `let item = show(..).await`, applying the result still forces a hop back into
  the sync loop. Async adds a runtime and *still* needs the sync re-entry.

The thing async offers — fire, forget, resolve later, caller-context captured —
is exactly what the makepad **action queue** already gives natively. So routing
is an action-queue + opaque `Tag`, not a future.

## Architecture

The service is a dismiss authority over one active surface. It knows two things:
**registration** ("I'm the active popup") and **kill triggers** (Esc /
focus-loss / deactivate / any system action / a press the active popup's own
hit-test ignored → outside-click). On any trigger it tells the active surface to
`hide`. Single-active falls out: registering a new surface hides the prior.

It does **not** know geometry, items, marking-vs-popup interaction, commit
routing, `Tag`, or compositing. Those belong to the surface, the Presenter, and
the caller.

Three separated concerns:

1. **`PopupRoot`** — dismiss authority. Single-active slot + kill triggers.
   Blind to everything else.
2. **`Presenter`** — the backing surface. Owns *where pixels land* and the
   event-space they arrive in. Platform-cfg picks the backing. Blind to *what*
   is drawn.
3. **content** (`MenuPopup` / `RadialPopup`) — geometry + items + draw. Blind to
   *which* backing it got — draws in its own origin-relative space either way.

### Module layout

New `popup/` module:

- `popup/root.rs` — `PopupRoot`, the authority. Single active slot,
  `show_at`/close, the light-dismiss route loop, `is_open()`.
- `popup/base.rs` — the `Popup` trait + shared types (`PopupResult`,
  `PopupClosed`, `PopupVerdict`, `PopupPlacement`) + `is_light_dismiss(event)`.
- `popup/presenter.rs` — `Presenter`, the content-blind backing. `#[cfg]` picks
  the DComp compositor window (Windows) or the in-app overlay layer (web + all
  else). Owns the draw target + the event-space translation. This is where the
  DComp / root-sibling-pass code lives — once, not per-surface.
- `popup/menu.rs` — `MenuPopup`, the linear card content.
- `popup/radial.rs` — `RadialPopup`, the wedge content.
- `popup/marking.rs` — `MarkingCore`, the demoted shared interaction helper (the
  marking / popup-latch / armed state machine both surfaces embed). NOT part of
  the authority — a reusable piece for authoring a surface kind.

`radial.rs` and `app_menu.rs` are deleted; their SDF material and geometry move
into `popup/radial.rs` / `popup/menu.rs` / `popup/marking.rs`. The DComp
compositing that lived inside the radial widget moves *out* of the content into
`popup/presenter.rs`, shared by both surfaces.

## The contract

```rust
// popup/base.rs
/// What a closed popup reports. `Invoked` carries the chosen item's id;
/// `Dismissed` is any light-dismiss (Esc / outside / blur / superseded).
pub enum PopupResult { Invoked(LiveId), Dismissed }

/// Emitted into the action queue on every close. The opener filters the queue
/// for its own `tag`; PopupRoot never inspects `tag` or `result`.
pub struct PopupClosed { pub tag: LiveId, pub result: PopupResult }

/// A surface's answer to one event.
pub enum PopupVerdict {
    Consumed, // the surface handled it (hover move, arm, in-surface press)
    Ignored,  // not for the surface (a press here => outside-click => dismiss)
    Closed,   // the surface committed or self-dismissed; it emitted PopupClosed
}

pub trait Popup {
    /// Drive one event. The surface owns its geometry + interaction; on a
    /// commit or self-dismiss it emits `PopupClosed{tag,..}` and returns `Closed`.
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict;
    /// PopupRoot forces the surface shut (light dismiss). The surface emits
    /// `PopupClosed{tag, Dismissed}` and tears down.
    fn hide(&mut self, cx: &mut Cx);
    fn tag(&self) -> LiveId;
}
```

```rust
// popup/root.rs — the whole "system"
pub struct PopupRoot { open: Option<PopupKind> }  // one active surface, or none

impl PopupRoot {
    /// Show `surface` at `anchor` with `placement`, hiding any prior open popup
    /// first (single-active). `surface` already carries its `tag`, items, and
    /// compositing choice.
    pub fn show_at(&mut self, cx, surface: PopupKind, anchor: DVec2, placement: PopupPlacement);
    pub fn is_open(&self) -> bool;

    /// App calls this once per event. The single seam.
    pub fn route(&mut self, cx: &mut Cx, event: &Event) {
        let Some(p) = self.open.as_mut() else { return };
        if is_light_dismiss(event) {                 // Esc / focus-loss / deactivate / system
            p.hide(cx);
            self.open = None;
            return;
        }
        match p.handle(cx, event) {
            PopupVerdict::Closed => self.open = None, // surface already emitted PopupClosed
            PopupVerdict::Ignored if is_primary_press(event) => {
                p.hide(cx); self.open = None;          // outside-click
            }
            _ => {}
        }
    }
}
```

`PopupKind` is a small enum wrapper (`Menu(MenuPopup) | Radial(RadialPopup)`) so
the one active slot is monomorphic without `Box<dyn>` (consistent with the
project's "enum over trait-object" preference). The `Popup` trait's three methods
dispatch through it.

### Light dismiss

`is_light_dismiss(event)` returns true for: `KeyDown` Escape; window
focus-lost / app-deactivate; and any incoming system action that should collapse
transient UI. Outside-click is **not** in this set — it is derived: a primary
press that the active surface's own hit-test returns `Ignored` for. The surface
claims what's inside it; anything it ignores, when it's a press, is outside and
kills it. No rect is handed to the authority.

### Placement

`show_at(surface, anchor, placement)` — `anchor` is a screen point/rect the
caller computes; `placement: PopupPlacement` is a preferred side (WinRT
`FlyoutPlacementMode`-style: `Below`, `Right`, `Auto`, …). The surface runs its
own on-screen flip/snap against its bounds — the radial's existing edge-snap "C"
fan is exactly this, generalised. The authority does not place; it only hosts.

## Presenter

The content-blind backing surface. It answers exactly one question — *where do
this popup's pixels land, and in what coordinate space do its events arrive* —
and nothing about *what* is drawn. This is the previously-deferred feature, now
first-class.

**Backing = platform cfg, not a per-surface flag.** There is no
`should_constrain_to_root_bounds` on content anymore:

- **Windows** → always a DComp compositor window (root-sibling pass-owner, per
  the proven `radial-popup-arch`). Every popup — menu *and* radial — can bloom
  past the app edge for free.
- **web + all other platforms** → an in-app overlay layer (no OS child windows;
  clamped to app bounds as a fallback).

`#[cfg]` selects the backing in `popup/presenter.rs`. Content never names DComp,
never learns which backing it got.

**The seam is coordinate space + draw target.** The Presenter:

1. hands content a draw context whose origin is `(0,0)` of *the Presenter's own
   space* — content draws origin-relative, identically in either backing;
2. translates every incoming event into that same space *before* content
   hit-tests — the Presenter now owns the aligned-parent-hit-rect offset gotcha
   (already solved for radial), per-backing. In-app overlay: translate by the
   overlay layer's origin. DComp: translate from the child window's coord space.

So content's `handle` / `item_at` / `contains` / draw code is written once
against origin-relative coordinates and works unchanged under both backings.

```rust
// popup/presenter.rs
/// The content-blind backing. Owns compositing + event-space; blind to content.
pub struct Presenter { /* #[cfg] backing: DComp window | in-app overlay */ }

impl Presenter {
    /// Place the backing at `anchor`/`placement` in screen space and size it to
    /// the content's bounds. Platform-cfg picks DComp vs in-app overlay.
    pub fn open(&mut self, cx: &mut Cx, anchor: DVec2, placement: PopupPlacement, size: DVec2);
    /// Tear the backing down (close the DComp window / drop the overlay layer).
    pub fn close(&mut self, cx: &mut Cx);
    /// Translate a raw event into the Presenter's origin-relative space, or
    /// `None` if it targets a different window/layer entirely.
    pub fn localize(&self, event: &Event) -> Option<Event>;
    /// A draw context anchored at the Presenter's (0,0) for content to draw into.
    pub fn draw_target(&mut self, cx: &mut Cx) -> /* origin-relative draw cx */;
}
```

A `PopupKind` surface holds its `Presenter`, calls `presenter.localize(event)`
at the top of its `handle`, hit-tests in origin space, and draws into
`presenter.draw_target(cx)`. `PopupRoot` stays compositing-agnostic — it only
sees `PopupVerdict`, never the Presenter.

## The two surfaces

Both implement `Popup`; both embed a `MarkingCore`; both hold a `Presenter` and
draw through it. They differ **only** in geometry + draw. Compositing is no
longer a per-surface axis — the Presenter owns it, platform-cfg (Windows DComp /
in-app overlay), identical for both.

### `MenuPopup` (linear card)

Ported from `app_menu.rs`. Geometry: a pure `LinearGeom` (`panel_rect` /
`row_at`, measured card width), owned by the surface and unit-tested directly.
Serves the logo dropdown, the caption burger, context menus, and the inspector
picker. On Windows it now gets the DComp backing too — a dropdown near the screen
bottom can overflow the app edge for free, where before it clipped.

### `RadialPopup` (wedge)

Ported from `radial.rs`. Geometry: hub dead-zone + wedge `index_at`, the pure
`RadialLayout` kept and unit-tested as-is; the edge-snapped partial "C" fan
stays as the on-web / in-app-overlay fallback when the disc can't bloom past
bounds. The DComp compositing that lived in the old radial widget is gone from
here — it moved into `popup/presenter.rs`, shared.

### `MarkingCore` (shared interaction helper)

The demoted state machine both surfaces embed. Geometry-free — the surface
resolves hits and feeds it:

```rust
pub struct MarkingCore {
    open: bool,
    tag: LiveId,
    items: Vec<PopupItem>,          // was RadialItem: id/label/icon/danger/enabled
    pressed: bool,                  // a button is held (marking candidate)
    dragged: bool,                  // passed DRAG_THRESHOLD -> marking mode
    popup: bool,                    // latched click-to-pick mode
    armed: Option<usize>,           // item under the cursor (hover/arm) — surface draws it
    press_pos: DVec2,               // drag-distance origin (only raw coords here)
}
```

The surface calls `pointer_move(cursor, hit)`, `release(hit, on_surface)`,
`click(hit, on_surface)` with `hit = surface.item_at(cursor)` (enabled-agnostic
slot) and `on_surface = surface.contains(cursor)`. Outcomes fall out of
`(hit, items[i].enabled, on_surface)`: `Some(i)`+enabled → commit; `Some(i)`+
disabled → stay open; `None` → cancel. `flick` (ride-past-rim) is geometry — the
radial surface computes it and passes it in; it is not a `MarkingCore` field.

`RadialItem` renames to shared `PopupItem` (already generic — id/label/icon/
danger/enabled).

## Routing — Tag + action queue

No central switchboard. The surface, on commit or dismiss, emits
`PopupClosed{tag, result}` into `cx`'s action queue. Each opener filters the
queue for **its own tag** in its own handling path. `App` is not a router; it
only pumps `PopupRoot::route` once per event and hosts the openers.

Example — inspector element-picker, the first generic consumer (it currently
draws its list inline, growing the panel frame; this rips that out):

```rust
// inspector opens: tag = the field's own LiveId (dynamic key rides in the tag)
popup_root.show_at(
    PopupKind::Menu(MenuPopup::new(field_id, rows)),
    anchor, PopupPlacement::Below,
);

// inspector's own action handling, later:
if let Some(PopupClosed { result: PopupResult::Invoked(item), .. })
    = popup_closed(actions).filter(|c| c.tag == field_id)
{
    self.set_field(field_id, item);
}
```

The opener that needs cross-tree placement it can't compute itself (a child
widget like the inspector) emits a small `OpenPicker { field, anchor, rows }`
action that `App` turns into one `show_at` call — App relays the *open*
(placement is a composition-root concern), but the *outcome* comes back directly
via the action queue, filtered by tag. The thing we are killing — App knowing
every caller's outcome mapping — stays killed.

## App-side collapse

- Delete `radial.rs`, `app_menu.rs`.
- Delete the `MenuOwner` enum and `set_app_menu_owner`. The caption-burger glow
  becomes caller-local: the burger opener lights its glow on `show_at` and drops
  it when it sees `PopupClosed{tag = burger}` (any result). Host stays blind.
- Replace the two `is_open()`-gated `handle()` calls + two outcome matches +
  manual reset with the single `popup_root.route(cx, event)`.
- Keep the `WindowDragQuery` client-ize-while-open hook, now reading
  `popup_root.is_open()`.
- The openers (logo, burger, canvas node-menu) stay where they are but call
  `show_at` and filter `PopupClosed` by their tag.

## Testing

- `MarkingCore` — port the existing `RadialCore` / `AppMenuCore` unit tests
  (tap-opens-popup, hold-drag-commits, flick, esc, disabled no-op, outside
  cancel). They already exist and are geometry-free once hits are fed in.
- `RadialLayout` — keep the existing pure geometry tests (edge-snap "C" fan, all
  wedges reachable, corner quarter) verbatim.
- `LinearGeom` — port `row_at` / `panel_rect` tests.
- `PopupRoot` — new unit tests for the authority alone, with a stub `Popup`:
  single-active (a second `show_at` hides the first, emitting its
  `PopupClosed{Dismissed}`), each light-dismiss trigger closes + emits
  `Dismissed`, an `Ignored` primary press closes, a `Consumed` event does not.
- `Presenter` — `localize` is pure and unit-testable: feed a raw event at a
  known window/overlay origin, assert it comes back in origin-relative space (and
  `None` for a foreign window). The DComp-vs-overlay backing itself is verified
  end-to-end, not in a unit test (it's platform-cfg + compositor).
- End-to-end verify by driving the app **on Windows**: open the inspector picker
  (DComp backing); open another popup (prior dies); Esc / click-out / alt-tab
  each dismiss the active one; open a dropdown near the screen bottom and confirm
  it overflows the app edge instead of clipping.

## Resolved open item

The previously-deferred feature is the **Presenter** (see
[Presenter](#presenter)): a content-blind backing that owns *where pixels land*,
platform-cfg (Windows DComp / in-app overlay elsewhere), lifted out of the
content surfaces. It attaches to the surfaces without touching `PopupRoot`'s
dismiss-authority role, exactly as the contract was built to absorb. Nothing is
left open; the design is settled.
