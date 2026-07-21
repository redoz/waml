### Task 3: `popup/radial.rs` — `RadialLayout` (moved) + `RadialPopup` widget + `impl Popup`

Port the wedge surface. The pure `RadialLayout` geometry + its tests move **verbatim** from `radial.rs`. The `Radial` widget becomes `RadialPopup`: same shaders and draw, but its `RadialCore` is replaced by an embedded `MarkingCore` + `RadialLayout` (geometry the surface owns and feeds into the core), and `handle` returns a `PopupVerdict`.

**Files:**
- Create: `crates/waml-editor/src/popup/radial.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (uncomment `pub mod radial;`)
- Reference (source of verbatim moves; NOT edited here — deleted in Task 7): `crates/waml-editor/src/radial.rs`

**Interfaces:**
- Consumes: `crate::popup::base::{PopupItem, Popup, PopupVerdict, PopupResult, is_primary_press}`; `crate::popup::marking::{MarkingCore, MarkOutcome}`; `crate::icons::{Icon, IconSet}`; `makepad_widgets::*`.
- Produces (Task 6 relies on these):
  - `pub struct RadialLayout { pub arc_start: f64, pub span: f64, pub n: usize }` + `full`/`snap`/`mid`/`wedge_bounds`/`index_at` — identical API to today's `radial::RadialLayout`.
  - `pub const HUB_RADIUS: f64`, `pub const DISC_RADIUS: f64`, `pub const EDGE_MARGIN: f64`, `pub const DRAG_THRESHOLD: f64 = 12.0`.
  - `pub struct RadialPopup` (a `#[derive(Script, ScriptHook, Widget)]` widget) with:
    - `pub fn open_marking(&mut self, cx: &mut Cx, center: DVec2, bounds: Rect, items: Vec<PopupItem>, time: f64)` — the node right-press open (was `Radial::open`).
    - `pub fn open_popup(&mut self, cx: &mut Cx, center: DVec2, bounds: Rect, items: Vec<PopupItem>, time: f64)` — direct popup open.
    - `pub fn is_open(&self) -> bool`.
    - `impl Popup for RadialPopup` (`handle`, `reset`).
  - DSL: `mod.widgets.RadialPopup` registered (was `mod.widgets.Radial`).

---

- [ ] **Step 1: Move `RadialLayout` + geometry helpers + their tests verbatim**

Create `crates/waml-editor/src/popup/radial.rs`. Copy **verbatim** from `radial.rs`:
- The module doc header idea (rewrite the first line to name `RadialPopup`).
- `HUB_RADIUS`, `DISC_RADIUS` (`radial.rs:17-26`), `EDGE_MARGIN` (`:55-59`), `DRAG_THRESHOLD` (`:215-221`), `BLOOM_SECS` (`:557-562`).
- `RadialLayout` struct + `Default` + the whole `impl RadialLayout` (`radial.rs:68-182`) — unchanged.
- `wedge_index` / `resolve_in` / `resolve_target` helper fns (`radial.rs:184-213`) — unchanged; the tests use them.
- The entire `#[cfg(test)] mod tests` block (`radial.rs:820-1110`) **except** the `RadialCore` state-machine tests (`tap_opens_persistent_popup_then_click_commits`, `begin_popup_*`, `hold_drag_*`, `flick_*`, `popup_click_*`, `esc_cancels`, `marking_release_in_hub_cancels`, `popup_click_on_disabled_*`, `click_into_blocked_region_cancels` — those moved to Task 2's `marking.rs`). KEEP the pure geometry tests: `n4_cardinal_directions_*`, `n2_*`, `n3_*`, `n5_and_n6_*`, `hub_dead_zone_*`, `wrap_around_*`, `disabled_wedge_resolves_to_none`, `resolve_target_none_in_hub`, `snap_*`, `partial_arc_keeps_all_wedges_reachable`. Change the test-module helper `fn item(..) -> RadialItem` to build `PopupItem` (import `crate::popup::base::PopupItem`) since `resolve_*` now take `&[PopupItem]`.

Change every `RadialItem` reference in the moved code to `PopupItem`. `resolve_in`/`resolve_target` signatures become `&[PopupItem]`.

- [ ] **Step 2: Run the moved geometry tests (fail — module not yet wired)**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::radial`
Expected: FAIL to compile (module not declared, `Popup`/widget parts absent). This step just confirms the file is picked up once wired; if it already compiles the geometry portion, the geometry tests should PASS. Wire the module now: in `mod.rs` uncomment `pub mod radial;`. Re-run — the geometry tests PASS, the widget parts below still absent so overall build fails until Step 4.

- [ ] **Step 3: Move the shaders + widget struct verbatim, renamed**

Copy the `script_mod! { .. }` block from `radial.rs:390-555` into `popup/radial.rs`. Two renames inside it:
- `mod.widgets.RadialBase = #(Radial::register_widget(vm))` → `#(RadialPopup::register_widget(vm))`.
- `mod.widgets.Radial = set_type_default() do mod.widgets.RadialBase{ .. }` → `mod.widgets.RadialPopup = set_type_default() do mod.widgets.RadialPopupBase{ .. }` (rename `RadialBase` → `RadialPopupBase` in both lines).
- The three `mod.draw.RadialWedge` / `RadialDisc` / `RadialHub` shader blocks (`radial.rs:409-535`) move **unchanged**.

Copy the widget struct `radial.rs:566-608` renamed `Radial` → `RadialPopup`, with ONE field change: replace `#[rust] core: RadialCore,` with:

```rust
    #[rust]
    mark: crate::popup::marking::MarkingCore,
    #[rust]
    center: DVec2,
    #[rust]
    bounds: Rect,
    #[rust]
    layout: RadialLayout,
```

Keep `start: f64` and `next_frame: NextFrame` and all `#[live] draw_*` / `icons` fields verbatim.

Copy the `impl Widget for RadialPopup` block (`radial.rs:610-619`) unchanged (empty `handle_event`, `draw_walk` → `self.draw(cx)`).

- [ ] **Step 4: Rewrite the inherent open/handle/draw methods against `MarkingCore` + `RadialLayout`**

The old `Radial` inherent methods (`radial.rs:621-818`) called `self.core` (a `RadialCore` that owned geometry). Now the surface owns geometry (`layout`/`center`/`bounds`) and drives the geometry-free `MarkingCore`. Write:

```rust
#[allow(dead_code)]
impl RadialPopup {
    pub fn is_open(&self) -> bool {
        self.mark.is_open()
    }

    /// Node right-press open: marking mode at `center`, fan snapped into `bounds`.
    pub fn open_marking(&mut self, cx: &mut Cx, center: DVec2, bounds: Rect, items: Vec<PopupItem>, time: f64) {
        self.center = center;
        self.bounds = bounds;
        self.layout = RadialLayout::snap(center, bounds, DISC_RADIUS, items.len());
        self.mark.begin_marking(center, items, DRAG_THRESHOLD);
        self.start = time;
        self.next_frame = cx.new_next_frame();
        self.draw_wedge.redraw(cx);
    }

    /// Direct popup open (a left-click open), same snapped fan.
    pub fn open_popup(&mut self, cx: &mut Cx, center: DVec2, bounds: Rect, items: Vec<PopupItem>, time: f64) {
        self.center = center;
        self.bounds = bounds;
        self.layout = RadialLayout::snap(center, bounds, DISC_RADIUS, items.len());
        self.mark.begin_popup(items, DRAG_THRESHOLD);
        self.start = time;
        self.next_frame = cx.new_next_frame();
        self.draw_wedge.redraw(cx);
    }

    fn tick(&mut self, cx: &mut Cx, event: &Event) {
        if self.next_frame.is_event(event).is_some() && self.mark.is_open() {
            self.next_frame = cx.new_next_frame();
            self.draw_wedge.redraw(cx);
        }
    }

    /// Resolve the wedge under `cursor` (enabled-agnostic) for the current fan.
    fn hit(&self, cursor: DVec2) -> Option<usize> {
        self.layout.index_at(self.center, cursor)
    }

    /// True when `cursor` is in the hub dead-zone or beyond the rim — the
    /// radial "outside" (a click there cancels).
    fn outside(&self, cursor: DVec2) -> bool {
        let r = (cursor - self.center).length();
        r < HUB_RADIUS || r > DISC_RADIUS
    }
}

impl Popup for RadialPopup {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.mark.is_open() {
            return PopupVerdict::Consumed;
        }
        self.tick(cx, event);
        let verdict = match event {
            Event::MouseMove(e) => {
                self.mark.pointer_move(e.abs, self.hit(e.abs));
                self.draw_wedge.redraw(cx);
                PopupVerdict::Consumed
            }
            // Marking release (secondary button let up after a press-drag).
            Event::MouseUp(e) if e.button.is_secondary() => {
                map_outcome(self.mark.release(self.hit(e.abs)))
            }
            // Popup mode: a PRIMARY press selects a wedge immediately. A press in
            // the hub / beyond the rim is the radial "outside" — but the disc
            // fills the whole overlay, so treat an outside press as `Ignored`
            // (PopupRoot dismisses) rather than a self-cancel, keeping the
            // outside-click path uniform with the linear surface.
            Event::MouseDown(e) if e.button.is_primary() && self.mark.is_popup() => {
                if self.outside(e.abs) {
                    PopupVerdict::Ignored
                } else {
                    map_outcome(self.mark.click(self.hit(e.abs), false))
                }
            }
            _ => PopupVerdict::Consumed,
        };
        if let PopupVerdict::Closed(_) = verdict {
            self.draw_wedge.redraw(cx);
        }
        verdict
    }

    fn reset(&mut self) {
        self.mark.close();
    }
}

/// Map the marking machine's outcome to a surface verdict. `None` (still open) =
/// Consumed; a commit/cancel = Closed with the matching result.
fn map_outcome(o: MarkOutcome) -> PopupVerdict {
    match o {
        MarkOutcome::Committed(id) => PopupVerdict::Closed(PopupResult::Invoked(id)),
        MarkOutcome::Cancelled => PopupVerdict::Closed(PopupResult::Dismissed),
        MarkOutcome::None => PopupVerdict::Consumed,
    }
}
```

Note the outside-press decision: the wedge disc's clickable area is angular over the whole overlay, so an unclaimed primary press (hub/rim) returns `Ignored`; `PopupRoot` then dismisses. This preserves the old "click hub/outside cancels" behavior (a `Dismissed`) while routing it through the single outside-click seam. The old `RadialCore.click`'s hub-cancel is kept as `outside()` → `Ignored`; the blocked-arc-region cancel (a `None` hit inside the disc) still routes through `click(None, false)` → `Cancelled` → `Closed(Dismissed)`.

- [ ] **Step 5: Rewrite `draw` against the owned geometry**

Move the old `Radial::draw` body (`radial.rs:698-817`) with these substitutions:
- `self.core.center()` → `self.center`; `self.core.items()` → `self.mark.items()`; `*self.core.layout()` → `self.layout`; `self.core.is_open()` → `self.mark.is_open()`.
- `let armed = self.core.armed;` → `let armed = self.mark.armed();`.
- The `flick` draw-state: `MarkingCore` has no `flick`. Compute it locally at the top of the per-item loop from geometry (the old `RadialCore::pointer_move` formula): a wedge is "flicked" when the cursor rode past the rim over the armed wedge while dragging. Since `draw` doesn't have the live cursor, drop the flick state to the arm state — i.e. replace the `state` ladder:
  ```rust
  let state = if !it.enabled {
      0.0
  } else if armed == Some(i) {
      2.0
  } else {
      0.0
  };
  ```
  (Flick was a purely cosmetic brighter highlight on ride-past-rim; dropping it in plan 1 is acceptable and removes the only `MarkingCore` field the spec explicitly excluded. If the flick shimmer must be preserved, store a `#[rust] flick: bool` on `RadialPopup` set in `handle`'s `MouseMove` arm — `self.flick = e.button/geometry`... — and read it here; leave that as a follow-up, not plan 1.)
- The arm-highlight now gates on `it.enabled` here (Task 2 note #1): the `if !it.enabled { 0.0 }` branch already handles it, so no extra guard is needed — armed being raw is fine because a disabled slot short-circuits to `0.0`.

- [ ] **Step 6: Run the geometry tests + build**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::radial && cargo build -p waml-editor`
Expected: PASS (the moved geometry tests) + clean build. `RadialPopup` will warn unused until Task 6/7 — the `#[allow(dead_code)]` covers it.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/popup/radial.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): RadialPopup surface (RadialLayout moved, MarkingCore-driven)

Wedge surface ported off RadialCore onto the shared MarkingCore + an
owned RadialLayout; shaders/draw moved verbatim from radial.rs. impl Popup
returns PopupVerdict; hub/rim press -> Ignored (outside-click seam). Pure
RadialLayout geometry tests kept; flick shimmer dropped for plan 1.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
