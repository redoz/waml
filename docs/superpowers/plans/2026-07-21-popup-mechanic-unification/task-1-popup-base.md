### Task 1: `popup/` module skeleton + `popup/base.rs`

The shared contract types + the trait every surface implements + the two pure event predicates. No surface behavior lands here — this is the vocabulary Tasks 2–8 speak.

**Files:**
- Create: `crates/waml-editor/src/popup/mod.rs`
- Create: `crates/waml-editor/src/popup/base.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod popup;` — see Step 1)

**Interfaces:**
- Consumes: nothing (first task). Uses `makepad_widgets::*` (`LiveId`, `Event`, `KeyCode`, `MouseButton`) and `crate::icons::Icon`.
- Produces (later tasks rely on these EXACT names/signatures):
  - `pub struct PopupItem { pub id: LiveId, pub label: String, pub icon: crate::icons::Icon, pub danger: bool, pub enabled: bool }` — `#[derive(Clone, Debug)]`.
  - `pub enum PopupResult { Invoked(LiveId), Dismissed }` — `#[derive(Clone, Debug, PartialEq)]`.
  - `pub enum PopupVerdict { Consumed, Ignored, Closed(PopupResult) }` — `#[derive(Clone, Debug, PartialEq)]`.
  - `pub trait Popup { fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict; fn reset(&mut self); }`
  - `pub fn is_light_dismiss(event: &Event) -> bool`
  - `pub fn is_primary_press(event: &Event) -> bool`

**Notes for the implementer:**
- `PopupItem` is the old `radial::RadialItem` renamed and moved. It stays field-identical so the old item-builder functions port with a type-name swap only.
- `is_light_dismiss` = Escape key-down **plus** window-focus-loss / app-deactivate. It does NOT include outside-click (that is derived in `PopupRoot::route` from an `Ignored` primary press — Task 6). Match the fork's event variants exactly; the existing code uses `Event::KeyDown(ke)` with `ke.key_code == KeyCode::Escape` and `Event::WindowLostFocus(_)` (see `app_menu.rs:418,422`). Grep the fork for any additional deactivate variant (`ActivateWindow`/`AppLostFocus`) and fold it in if present — otherwise `WindowLostFocus` alone matches today's behavior.
- `is_primary_press` = `matches!(event, Event::MouseDown(e) if e.button.is_primary())` (mirrors `radial.rs:685`, `app_menu.rs:413`).
- The `Popup` trait deliberately has NO `tag()` method: `PopupRoot` owns the active tag in its slot (Task 6), so the surface never needs to report it. `reset()` returns the surface to its closed state without emitting anything (`PopupRoot` emits `Closed`). This is the plan's realization of the spec's `hide` — renamed `reset` because emission is `PopupRoot`'s job, not the surface's.

---

- [ ] **Step 1: Create the module skeleton and register it**

Create `crates/waml-editor/src/popup/mod.rs`:

```rust
//! The generic single-active popup mechanic. `PopupRoot` (an authority widget)
//! hosts at most one active ephemeral surface and runs universal light-dismiss;
//! `MenuPopup` (linear card) and `RadialPopup` (wedge) are the two surface kinds,
//! both driven through the `Popup` trait and both embedding the shared
//! `MarkingCore`. See `docs/superpowers/specs/2026-07-21-generic-popup-mechanic-design.md`.

pub mod base;
// Filled by later tasks:
// pub mod marking;   // Task 2
// pub mod radial;    // Task 3
// pub mod menu;      // Task 4
// pub mod presenter; // Task 5
// pub mod root;      // Task 6
```

Register the module. `crates/waml-editor/src/main.rs` is the crate root; find the block of top-level `mod` declarations (near the other `mod radial;` / `mod app_menu;` lines) and add, in alphabetical position:

```rust
mod popup;
```

Do NOT remove `mod radial;` / `mod app_menu;` yet — they are deleted in Task 7.

- [ ] **Step 2: Write the failing test for the predicates**

Create `crates/waml-editor/src/popup/base.rs` with only the test module first, so the run fails to compile (functions absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::*;

    #[test]
    fn escape_keydown_is_light_dismiss() {
        let e = Event::KeyDown(KeyEvent {
            key_code: KeyCode::Escape,
            ..Default::default()
        });
        assert!(is_light_dismiss(&e));
        assert!(!is_primary_press(&e));
    }

    #[test]
    fn primary_mousedown_is_a_primary_press_not_a_dismiss() {
        let e = Event::MouseDown(MouseDownEvent {
            button: MouseButton::PRIMARY,
            ..Default::default()
        });
        assert!(is_primary_press(&e));
        assert!(!is_light_dismiss(&e));
    }
}
```

- [ ] **Step 3: Run it to confirm it fails**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::base -- --nocapture`
Expected: FAIL — compile error, `is_light_dismiss` / `is_primary_press` not found.

(If `KeyEvent`/`MouseDownEvent`/`MouseButton::PRIMARY` field names differ in the fork, fix the test constructors to match — grep `radial.rs` / `app_menu.rs` for how these events are matched to learn the exact shapes. The `..Default::default()` form works only if the event structs derive `Default`; if not, construct them the way the fork's own tests do, or drop these two predicate tests to a single `is_primary_press` smoke test built from a real event the fork exposes.)

- [ ] **Step 4: Write the contract types + predicates above the test module**

Prepend to `crates/waml-editor/src/popup/base.rs`:

```rust
//! The popup contract: the item shape, the closed-result, the per-event verdict,
//! the surface trait, and the two pure event predicates the authority routes on.

use crate::icons::Icon;
use makepad_widgets::*;

/// One selectable entry. The surface owns no command semantics — it reports `id`
/// back on commit and the opener maps it. (Renamed + moved from `radial::RadialItem`.)
#[derive(Clone, Debug)]
pub struct PopupItem {
    pub id: LiveId,
    pub label: String,
    pub icon: Icon,
    /// Danger-token hue across all states.
    pub danger: bool,
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
}

/// What a closed popup reports. `Invoked` carries the chosen item's id; any
/// light-dismiss (Esc / outside / blur / superseded) reports `Dismissed`.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupResult {
    Invoked(LiveId),
    Dismissed,
}

/// A surface's answer to one event, returned from `Popup::handle`.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupVerdict {
    /// The surface handled it (hover move, arm, in-surface press).
    Consumed,
    /// Not for the surface. A *primary press* here is an outside-click: the
    /// authority turns it into a dismiss (see `PopupRoot::route`).
    Ignored,
    /// The surface committed or self-dismissed; the authority emits the matching
    /// `PopupRootAction::Closed` and clears the active slot.
    Closed(PopupResult),
}

/// Every surface kind implements this. The surface owns its geometry + marking
/// interaction; the authority owns the active slot, light-dismiss, and emission.
pub trait Popup {
    /// Drive one already-localized event; return the verdict.
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict;
    /// Return to the closed state WITHOUT emitting (the authority emits the
    /// `Closed` action). Called on any light-dismiss / supersede.
    fn reset(&mut self);
}

/// True for events that collapse transient UI regardless of pointer position:
/// Escape, and window focus-loss / app-deactivate. Outside-click is NOT here —
/// it is derived from an `Ignored` primary press in `PopupRoot::route`.
pub fn is_light_dismiss(event: &Event) -> bool {
    match event {
        Event::KeyDown(ke) if ke.key_code == KeyCode::Escape => true,
        Event::WindowLostFocus(_) => true,
        _ => false,
    }
}

/// True for a primary (left) button press.
pub fn is_primary_press(event: &Event) -> bool {
    matches!(event, Event::MouseDown(e) if e.button.is_primary())
}
```

- [ ] **Step 5: Run the tests to confirm they pass + the crate compiles**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::base && cargo build -p waml-editor`
Expected: PASS (2 tests) and a clean build. `PopupItem`/`Popup`/`PopupVerdict` will warn `never used` — that is expected until Task 2+; add `#![allow(dead_code)]` at the top of `base.rs` (the same convention `radial.rs:22` uses) to keep the build warning-clean.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/popup/mod.rs crates/waml-editor/src/popup/base.rs crates/waml-editor/src/main.rs
git commit -m "feat(popup): base contract types + light-dismiss predicates

PopupItem/PopupResult/PopupVerdict, the Popup surface trait, and the pure
is_light_dismiss/is_primary_press predicates that PopupRoot will route on.
First slice of the generic single-active popup mechanic (module skeleton).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
