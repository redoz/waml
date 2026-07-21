### Task 5: `popup/presenter.rs` — overlay-backed `Presenter`

The content-blind backing. In plan 1 it has the **overlay** backing only: events already arrive in main-window coordinates, so `localize` is the identity; its one real job is the in-app-overlay **clamp** (keep a card inside the window when it would overflow — the spec's "clamped to app bounds as a fallback"). This is the seam a later plan swaps for the Windows DComp backing without touching `PopupRoot` or the surfaces.

Keep this deliberately small — it is a seam, not a subsystem. `PopupRoot` (Task 6) calls `Presenter::localize` at the top of `route` and `Presenter::place` inside `show_at` (for the linear card; the radial does its own `RadialLayout::snap`).

**Files:**
- Create: `crates/waml-editor/src/popup/presenter.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (uncomment `pub mod presenter;`)

**Interfaces:**
- Consumes: `makepad_widgets::{Event, Rect, DVec2, dvec2}`.
- Produces (Task 6 relies on these):
  - `pub struct Presenter;` (unit — the overlay backing is stateless in plan 1).
  - `pub fn localize<'a>(&self, event: &'a Event) -> &'a Event` — identity in plan 1.
  - `pub fn place(anchor: DVec2, size: DVec2, bounds: Rect) -> DVec2` — clamp the top-left so `[anchor, anchor+size]` sits inside `bounds`; pin to the bounds' top-left if `size` exceeds `bounds` on an axis (associated fn — no `self`).

---

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-editor/src/popup/presenter.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::*;

    const WIN: Rect = Rect { pos: DVec2 { x: 0.0, y: 0.0 }, size: DVec2 { x: 1000.0, y: 800.0 } };

    #[test]
    fn card_fully_inside_is_unchanged() {
        let p = Presenter::place(dvec2(100.0, 60.0), dvec2(200.0, 300.0), WIN);
        assert_eq!(p, dvec2(100.0, 60.0));
    }

    #[test]
    fn card_overflowing_right_and_bottom_shifts_to_fit() {
        // anchor near the far corner; 200x300 card would spill past 1000x800.
        let p = Presenter::place(dvec2(900.0, 700.0), dvec2(200.0, 300.0), WIN);
        assert_eq!(p, dvec2(800.0, 500.0)); // shifted left/up so the box just fits
    }

    #[test]
    fn card_larger_than_bounds_pins_to_top_left() {
        let p = Presenter::place(dvec2(50.0, 50.0), dvec2(1200.0, 900.0), WIN);
        assert_eq!(p, dvec2(0.0, 0.0));
    }

    #[test]
    fn localize_is_identity_in_plan_one() {
        let ev = Event::KeyDown(KeyEvent { key_code: KeyCode::Escape, ..Default::default() });
        let out = Presenter.localize(&ev);
        // Same event, unchanged (overlay backing: no coordinate translation).
        assert!(matches!(out, Event::KeyDown(k) if k.key_code == KeyCode::Escape));
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::presenter`
Expected: FAIL — `Presenter` not found. (If `KeyEvent { ..Default::default() }` doesn't compile in the fork, mirror the constructor Task 1 settled on.)

- [ ] **Step 3: Write `Presenter` above the test module**

```rust
//! `Presenter` — the content-blind backing surface: owns *where pixels land* and
//! the event coordinate space, blind to *what* is drawn. Plan 1 ships only the
//! in-app **overlay** backing: events already arrive in main-window coordinates,
//! so `localize` is the identity, and the one real job is clamping a card inside
//! the window (the spec's app-bounds fallback). A later plan adds the Windows
//! DComp compositor-window backing behind this same API — `localize` gains a
//! child-window→main-window translation and `place` gains the out-of-window path,
//! with NO change to `PopupRoot` or the surfaces.

use makepad_widgets::*;

pub struct Presenter;

#[allow(dead_code)]
impl Presenter {
    /// Normalize an event into the space the surfaces hit-test in (main-window
    /// coords). Identity for the overlay backing; the DComp backing translates.
    pub fn localize<'a>(&self, event: &'a Event) -> &'a Event {
        event
    }

    /// Clamp a card's top-left so `[anchor, anchor+size]` stays inside `bounds`.
    /// If `size` exceeds `bounds` on an axis, pin to the bounds' near edge.
    pub fn place(anchor: DVec2, size: DVec2, bounds: Rect) -> DVec2 {
        let max_x = (bounds.pos.x + bounds.size.x - size.x).max(bounds.pos.x);
        let max_y = (bounds.pos.y + bounds.size.y - size.y).max(bounds.pos.y);
        dvec2(
            anchor.x.clamp(bounds.pos.x, max_x),
            anchor.y.clamp(bounds.pos.y, max_y),
        )
    }
}
```

- [ ] **Step 4: Run tests + build**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::presenter && cargo build -p waml-editor`
Expected: PASS (4 tests) + clean build. Uncomment `pub mod presenter;` in `mod.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/popup/presenter.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): content-blind overlay Presenter (identity localize + clamp)

Plan-1 overlay backing: localize is identity (events already in main-window
space), place() clamps a card inside window bounds (app-bounds fallback). The
seam a later plan swaps for the Windows DComp backing without touching
PopupRoot or the surfaces.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
