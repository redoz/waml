# Diagram View Seam — Design

**Date:** 2026-07-23
**Status:** Approved (autonomous mandate)
**Topic:** Introduce a logical seam between the waml-editor *app shell* and per-tab *document views*, so behavior sits in the component that owns it and future diagram kinds slot in cleanly.

---

## 1. Motivation

Today `App` (`crates/waml-editor/src/app.rs`, ~1822 lines) is a monolith. `sync_active_tab` branches on `TabKind` and `handle_actions` interleaves *every* behavior — shell chrome (project tree, burger menu, logo menu, start screen, shortcuts overlay, theme toggle) tangled with document behavior (canvas camera/select/expand, inspector edits, element picker, tool dock, selection toolbar). There is no boundary between "app UI that is always present" and "the view of the currently open document."

The mental model we are encoding:

- **Model panel** (the project tree) = permanent main-UI chrome. It belongs to the *shell*, not to any diagram.
- **UML Class Diagram** = a *feature* that is really its own **View**: it owns a canvas + an introspection panel (with the element/diagram picker) + a tool dock + a selection toolbar.
- **Opening an element** yields a **preview** that *looks* like the diagram view but is a **distinct View** — its own canvas and a panel similar to the introspection panel, minus the element selector. It may render very differently in the future.
- **Future kinds** — Sequence diagrams, Activity diagrams, and (eventually) raw OKF bundle visualizations — each become their own Diagram View in the same slot.

Goal: put the seam in *now* so the right behavior lives in the right component, without changing what the user sees.

## 2. Scope

**In scope**

- A `DocView` trait: the contract every per-tab document view implements.
- Two concrete views extracted from the current `TabKind` arms:
  - `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-picker + tool dock + selection toolbar).
  - `ClassifierPreviewView` — the single-element preview (focus canvas + inspector-without-picker, no tool dock).
- A shell-owned view registry: one `Box<dyn DocView>` per open tab, keyed by tab id, created by a factory that discriminates on `TabKind`.
- A `ViewOutcome` type: what a view hands back up to the shell each interaction (edit intents, preview-open requests, popup requests).
- Shell relays outcomes: applies `Op`s to the `Model`, opens previews via `OpenTabs`, and places cross-tree popups through `popup_root` on the view's behalf.

**Out of scope** (explicit non-goals)

- `Op::PlaceSet` layout write-back — separate follow-up already specced.
- Sequence / Activity / OKF views — this establishes the seam they will plug into; none are built here.
- Any dynamic *widget-subtree* instancing in the Makepad tree. This codebase is hand-rolled immediate-mode with **no** PortalList/TabBar/dynamic-mount precedent (see §4).
- Visual change. Screen is pixel-identical before and after.

## 3. The seam

```
                 ┌──────────────────────────── App (shell) ─────────────────────────────┐
                 │  owns: Model, OpenTabs, popup_root, project tree, caption bar,         │
                 │        start screen, shortcuts overlay, theme, hotkeys                 │
                 │                                                                        │
                 │   views: HashMap<TabId, Box<dyn DocView>>   (one live object per tab)  │
                 └───────────────┬────────────────────────────────────────────────────────┘
                                 │  &Model (read)          ▲  ViewOutcome (Ops / OpenPreview / PopupRequest)
                                 ▼                         │
                 ┌───────────── active DocView ────────────┴──────────────┐
                 │  ClassDiagramView  |  ClassifierPreviewView             │
                 │  owns LIVE per-tab state: camera, selection, expanded   │
                 │  renders through ONE shared body draw surface           │
                 └─────────────────────────────────────────────────────────┘
```

Rules:

1. Views render from a borrowed `&Model`. They never mutate `Model`, `OpenTabs`, or `popup_root` directly.
2. Views emit intent via `ViewOutcome`. The shell is the only place that applies edits, opens tabs, and places popups.
3. The shell owns everything permanent (chrome). Views own everything document-scoped.

## 4. Mounting decision — object-per-tab, one shared draw surface

Because the codebase has **no dynamic widget-instancing precedent** (widgets are hand-drawn with `draw_abs`, hit rects captured manually, `FingerUp` hit-tested), we do **not** mount a live Makepad subtree per tab. Instead:

- Each open tab owns a **plain Rust object** (`Box<dyn DocView>`) that holds its live state — camera, selection, expanded set.
- The single existing body widget (canvas + inspector region) is the **one shared draw surface**. On `sync_active_tab` the active view is asked to render itself into that surface.
- Switching tabs swaps which object drives the surface; per-tab state persists in the objects, not in mounted widgets.

This delivers per-tab state persistence (the "A / dynamic" decision) without unproven mounted-subtree machinery. It is the low-risk realization of "the view is bound per tab."

The **statusbar / selection toolbar folds into the active View** — it is document-scoped chrome (it reflects the current selection), so it lives with the view that owns the selection, not the shell.

## 5. Contract sketch

```rust
/// One open document tab's behavior + live state. Shell-owned, one per tab.
trait DocView {
    /// Render into the shared body surface from a read-only Model.
    fn sync(&mut self, cx: &mut Cx2d, ui: &WidgetRef, model: &Model);

    /// Consume shell actions routed to the active tab; return intent upward.
    fn handle(&mut self, cx: &mut Cx, actions: &Actions, model: &Model) -> ViewOutcome;

    /// Does this view drive the tool dock? (diagram: yes, preview: no)
    fn wants_tooldock(&self) -> bool;

    fn on_activate(&mut self, cx: &mut Cx) {}
    fn on_deactivate(&mut self, cx: &mut Cx) {}
}

/// What a view hands back to the shell per interaction.
struct ViewOutcome {
    ops: Vec<Op>,                    // edit intents applied by shell to Model
    open_preview: Option<String>,    // request shell to open element preview (key)
    popup: Option<PopupRequest>,     // request shell to place a cross-tree popup
}
```

`PopupRequest` mirrors the existing popup-relay pattern: the view describes the popup it wants; the shell places it via `popup_root` and routes the result back down on a later `handle`.

Factory (discriminates on `TabKind`):

```rust
fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram    => Box::new(ClassDiagramView::new(...)),
        TabKind::Classifier => Box::new(ClassifierPreviewView::new(tab.key.clone(), ...)),
    }
}
```

## 6. Migration path (behavior-preserving, per commit)

Each step compiles, tests green, screen identical:

1. **Extract shell-body accessor bundle.** Group the body widget refs (canvas, inspector, tool dock, selection toolbar) the current arms poke into one struct the future views borrow. No behavior change.
2. **Introduce `DocView` + `ViewOutcome` + factory.** Add the trait and empty registry alongside the existing `match`. Nothing calls it yet.
3. **Move the Diagram arm into `ClassDiagramView`.** Port `build_scene`, inspector-with-picker, tool dock, selection toolbar, and the canvas/inspector/picker/dock/toolbar branches of `handle_actions` into the view. Shell delegates the Diagram case to the view and relays its `ViewOutcome`.
4. **Move the Classifier arm into `ClassifierPreviewView`.** Port `build_focus_scene` and the classifier-focus branches. Shell delegates the Classifier case.
5. **Shell drives the registry.** Replace the `TabKind` `match` in `sync_active_tab` / `handle_actions` with "look up the active tab's view, delegate, relay outcome." Delete the now-dead monolith branches.

After step 5 the shell holds only chrome behavior; both document behaviors live in their views. Real `Op` application still flows through the shell's existing `Op` handling — the seam just moves *who requests* the ops.

## 7. Testing

- Each migration commit runs the existing suite (299+ unit tests) green.
- Visual parity check: build the worktree's own copy with `-Optimized`, screenshot-verify the class diagram, an opened element preview, and tab-switching between them — capture by explicit pid, never kill the user's running editor.
- Assert per-tab state persistence: switch away from a diagram with a panned camera + selection, switch back, state intact (now held in the view object).

## 8. Risks

- **Borrow tangle.** Views need `&Model` while the shell needs `&mut self`. Mitigation: `ViewOutcome` return-value channel — views never hold `&mut Model`; the shell applies ops after `handle` returns.
- **Popup routing regressions.** The relay indirection could drop a route. Mitigation: keep `popup_root.route` in the shell exactly as-is; only the *request* origin moves into the view.
- **Hidden shared state.** `nav_state` / `expanded` may be read by both chrome and document. Mitigation: step 1's accessor-bundle extraction surfaces every shared field before any behavior moves.
