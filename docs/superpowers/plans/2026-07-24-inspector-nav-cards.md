# Inspector Navigable Reference Cards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the native inspector's group MEMBERS and ASSOCIATIONS as one shared, compact, square-cornered, *clickable* reference card (`RefCardView`, replacing `RelationshipCardView`); a click repoints the inspector AND selects the node on the canvas. Swap the Group icon to a new Lucide `group` glyph, tighten section spacing, and nudge the collapsed select-box label baseline up.

**Architecture:** The pure read model (`inspector.rs`) gains element references so a click can resolve a target. A new pure-view Turtle widget (`ref_card.rs`, `recent_row.rs`-templated) backs both per-section FlatLists. Navigation is a reader on the `Inspector` widget that `class_diagram_view.rs` consumes to repoint the inspector and drive a new `Canvas::select_by_key`. Everything is native-only (`show_picker == true` FlatList path); the web/Svelte frontend and the `!show_picker` immediate-mode preview body are untouched.

**Tech Stack:** Rust, the redoz Makepad fork (`script_mod!` DSL widgets, SDF shaders, immediate-mode `DrawText`/`DrawColor`, `FlatList`), the Atlas HUD theme tokens (`atlas.*`).

## Global Constraints

- **Worktree isolation (hard rule):** ALL edits use ABSOLUTE paths under `C:\dev\waml\.worktrees\inspector-nav-cards\`. NEVER edit `C:\dev\waml\crates\...` (the main checkout) — a main-root path silently edits MAIN while the build runs the worktree's stale copy and "passes" as a false baseline.
- **Gate per task (each unit must pass on its own, run from the worktree root):**
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - The gate promotes `dead_code` / unused-field / unused-method to HARD errors. Never land a struct field, widget, or method that nothing reads until a later task — land producer + first consumer together, or the unit is red.
- **Native-only:** No `pnpm`/web steps. Web/Svelte and the wasm ABI are out of scope and MUST stay byte-identical.
- **Icon catalog invariant (do not violate):** for every glyph, `enum Icon` variant == `IconSet` struct field == DSL `IconSet` instance field == DSL `mod.draw.Icon*` shader == `IconSet::get` match arm == `Icon::ALL` entry == `Icon::label` match arm, ALL IN THE SAME ORDER, and every `90`-count assertion is bumped in lockstep. Add-only — never remove `SquareDashedTopSolid` (it stays in the catalog even after Group stops using it).
- **Shader gotcha:** `sdf.box(x, y, w, h, 0.0)` with a 0 radius DEGENERATES and floods in this fork. For sharp SQUARE corners use `sdf.rect`. Use `sdf.box` only with a real (>0) radius.
- **Widget-registration ordering (subtle, tests + review both MISS it):** a custom widget mounted as a DSL child is a DEAD, invisible, unqueryable node unless its `script_mod(vm)` is registered in `app.rs` BEFORE the consuming module's `script_mod` runs (`mod.widgets.*` resolves eagerly at `use`-time). `RefCardView` MUST register before `inspector_panel`, mirroring how `relationship_card`/`section_heading`/`attr_row`/`select_box` are wired at `app.rs` ~L1732–1740.
- **Visual verification is mandatory and pid-safe:** never screenshot or `Stop-Process` the editor by process NAME — that grabs/kills the user's own running `waml-editor`. Build and launch the worktree's OWN exe, capture the screenshot by the specific spawned PID, and stop only that PID. Green tests + code review do NOT catch the dead-widget or the visual-spacing class of bug.

---

## File Structure

- `crates\waml-editor\src\inspector.rs` — pure read model. Gains `ElementRef`, `members: Vec<ElementRef>`, `AssocRow.target_key`/`target_kind`, and a pure `subject_from` helper. Unit tests extended here.
- `crates\waml-editor\src\ref_card.rs` — NEW. `RefCardView`: pure-view (Task 2) then clickable (Task 3) Turtle widget. One square-cornered bordered card = element-kind lead icon + name (+ optional dim meta line).
- `crates\waml-editor\src\relationship_card.rs` — DELETED in Task 2 (superseded by `RefCardView`).
- `crates\waml-editor\src\inspector_panel.rs` — the `Inspector` widget. MEMBERS switches from a joined `Label` to a `RefCardView` FlatList; ASSOCIATIONS repoint their FlatList at `RefCardView`; a `navigate` reader lands (Task 3); the Group-icon map and section spacing/card constants are tuned (Tasks 4–5).
- `crates\waml-editor\src\canvas.rs` — `GraphCanvas` gains `pub fn select_by_key` (Task 3).
- `crates\waml-editor\src\class_diagram_view.rs` — `DocView::handle` consumes `inspector.navigate` → repoint + canvas-select (Task 3).
- `crates\waml-editor\src\icons.rs` — the icon catalog gains a Lucide `group` glyph (Task 4).
- `crates\waml-editor\src\select_box.rs` — collapsed selected-label baseline nudge (Task 5).
- `crates\waml-editor\src\main.rs` — module table: add `mod ref_card;`, remove `mod relationship_card;` (Task 2).
- `crates\waml-editor\src\app.rs` — `script_mod` registration order: add `ref_card`, remove `relationship_card` (Task 2).

---

### Task 1: Read model carries element references

**Files:**
- Modify: `crates\waml-editor\src\inspector.rs` (`ElementRef` new; `InspectorView.members`; `AssocRow`; `build_group_view`; `build_classifier_view`; tests)
- Modify: `crates\waml-editor\src\inspector_panel.rs` (the two sites that read `view.members` as `String`s)

**Interfaces:**
- Produces:
  - `pub struct ElementRef { pub key: String, pub kind: ElementKind, pub label: String }` (derives `Debug, Clone, PartialEq, Eq`).
  - `InspectorView.members: Vec<ElementRef>` (was `Vec<String>`).
  - `AssocRow.target_key: String`, `AssocRow.target_kind: ElementKind` (far endpoint; kind is `ElementKind::Node` for every association — edges connect nodes).
- Consumes: nothing from later tasks.

> Note on staying green: the new `AssocRow`/`ElementRef` fields are read by the `#[derive(Debug, Clone, PartialEq, Eq)]` impls AND by the extended unit tests in this task, so `dead_code` does not fire even though `inspector_panel.rs`/`class_diagram_view.rs` do not consume `target_key`/`target_kind` until Tasks 2–3.

- [ ] **Step 1: Write the failing tests**

In `crates\waml-editor\src\inspector.rs`, replace the body of the existing `group_projects_name_kind_and_members` test and add two new tests inside `mod tests`:

```rust
    #[test]
    fn group_projects_name_kind_and_members() {
        let model = mini_with_group();
        let view = build_view(&model, &Subject::Group("Sales".into())).unwrap();
        assert_eq!(view.title, "Sales");
        assert_eq!(view.kind_label, "Group");
        // Members are ElementRefs: node kind, key = member key, label = node title.
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        assert_eq!(view.members.len(), 2);
        assert_eq!(view.members[0].key, order);
        assert_eq!(view.members[0].kind, ElementKind::Node);
        assert_eq!(view.members[0].label, "Order");
        assert_eq!(view.members[1].key, customer);
        assert_eq!(view.members[1].kind, ElementKind::Node);
        assert_eq!(view.members[1].label, "Customer");
        assert!(view.attributes.is_empty());
        assert!(view.associations.is_empty());
        assert!(view.description.is_none());
    }

    #[test]
    fn association_target_resolves_to_far_endpoint() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Outgoing Order->Customer: far endpoint is Customer.
        assert_eq!(assoc.target_key, customer);
        assert_eq!(assoc.target_kind, ElementKind::Node);
        assert_eq!(assoc.other_label, "Customer");
    }

    #[test]
    fn incoming_association_target_is_the_source_node() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(customer)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Incoming (Customer is the target): far endpoint is the source, Order.
        assert_eq!(assoc.target_key, order);
        assert_eq!(assoc.target_kind, ElementKind::Node);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib inspector:: 2>&1 | rg -n "target_key|members|error\[|FAILED"`
Expected: compile errors (`no field target_key on AssocRow`, `expected String, found ElementRef`) — the read model does not carry references yet.

- [ ] **Step 3: Add `ElementRef` and extend the structs**

In `crates\waml-editor\src\inspector.rs`, immediately after the `ElementKind` enum (right after its closing `}`, ~L52), add:

```rust
/// A navigable reference to one diagram element: enough for the panel to
/// repoint (`key` + `kind`) and to label a card (`label`). Backs both member
/// and association cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef {
    pub key: String,
    pub kind: ElementKind,
    pub label: String,
}
```

In the `AssocRow` struct (~L93), add the two fields after `multiplicity`:

```rust
pub struct AssocRow {
    pub kind: String,         // RelationshipKind::as_str(), e.g. "associates"
    pub dir: AssocDir,        // orientation from the subject's point of view
    pub other_label: String,  // the far endpoint's title, falling back to its key
    pub role: String,         // far end's role, "" when unset
    pub multiplicity: String, // far end's multiplicity, "" when unset or trivial "1"
    pub target_key: String,   // the far endpoint's element key (the navigate target)
    pub target_kind: ElementKind, // the far endpoint's kind (Node for associations)
}
```

In the `InspectorView` struct (~L103), change the `members` field type + doc:

```rust
    /// Group member references; empty for every non-group subject.
    pub members: Vec<ElementRef>,
```

- [ ] **Step 4: Populate the new fields**

In `build_classifier_view` (~L318), change the `associations.push(AssocRow { ... })` to also carry the target. `far_key` is already in scope (`let far_key = if outgoing { &edge.target } else { &edge.source };`):

```rust
        associations.push(AssocRow {
            kind: edge.kind.as_str().to_string(),
            dir,
            other_label: node_title(model, far_key),
            role,
            multiplicity,
            target_key: far_key.clone(),
            target_kind: ElementKind::Node,
        });
```

In `build_group_view` (~L357), change the `members` mapping:

```rust
    let members = group
        .members
        .iter()
        .map(|k| ElementRef {
            key: k.clone(),
            kind: ElementKind::Node,
            label: node_title(model, k),
        })
        .collect();
```

- [ ] **Step 5: Fix the two `inspector_panel.rs` read sites**

In `crates\waml-editor\src\inspector_panel.rs`:

Add `ElementRef` to the `crate::inspector` import (~L30):

```rust
use crate::inspector::{
    build_view, effective_field, subject_to_index, AssocDir, AssocRow, ElementKind, ElementRef,
    ElementRow, FieldId, InspectorView, Subject,
};
```

In `fill_body_column` (the joined-line set, ~L919), map to labels:

```rust
            self.view.label(cx, ids!(body.members_lines)).set_text(
                cx,
                &view
                    .members
                    .iter()
                    .map(|m| m.label.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
```

In the immediate-mode `!show_picker` body (~L800), the members loop now iterates `ElementRef`:

```rust
            for m in &view.members {
                self.draw_label.draw_abs(cx, dvec2(x, y), &m.label);
                y += ROW_H;
            }
```

> `ElementRef` is now imported but only its `.label`/`.key`/`.kind` are read here; that is a real read, so no unused-import warning. (If clippy flags `ElementRef` as unused-import at THIS task because only `.label` is touched via the type inference on `view.members`, drop it from the `use` and keep it — re-add in Task 3 where `ElementRef` is named explicitly. Verify with the gate below and adjust.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib inspector::`
Expected: PASS, including `group_projects_name_kind_and_members`, `association_target_resolves_to_far_endpoint`, `incoming_association_target_is_the_source_node`.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean (no warnings, all tests pass).

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/inspector.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): read model carries element references (members + assoc targets)"
```

---

### Task 2: `RefCardView` widget replaces `RelationshipCardView`

**Files:**
- Create: `crates\waml-editor\src\ref_card.rs`
- Modify: `crates\waml-editor\src\main.rs` (`mod ref_card;` add, `mod relationship_card;` remove)
- Modify: `crates\waml-editor\src\app.rs` (register `ref_card` before `inspector_panel`; remove `relationship_card` registration)
- Modify: `crates\waml-editor\src\inspector_panel.rs` (both FlatLists point at `RefCardView`; MEMBERS becomes a FlatList; remove `RelationshipCardView` usage)
- Delete: `crates\waml-editor\src\relationship_card.rs`

**Interfaces:**
- Produces (`ref_card.rs`), pure-view for this task:
  - `pub struct RefCardView` (`#[deref] view: View`, `#[live] icons: IconSet`, `#[live] draw_icon: DrawColor`, `#[rust] icon: Option<Icon>`).
  - `impl RefCardView`: `pub fn set_icon(&mut self, cx: &mut Cx, icon: Icon)`, `pub fn set_name(&mut self, cx: &mut Cx, s: &str)`, `pub fn set_meta(&mut self, cx: &mut Cx, s: &str)` (empty string hides the meta line).
  - `impl RefCardViewRef`: the same three setters (borrow-guarded), plus `pub use ref_card::RefCardViewWidgetRefExt`-style `as_ref_card_view()` generated by the `Widget` derive.
- Consumes: `crate::inspector::{ElementKind}` and `crate::icons::{Icon, IconSet}`.

- [ ] **Step 1: Write the `ref_card.rs` widget**

Create `crates\waml-editor\src\ref_card.rs` with the pure-view card. Border is a SQUARE `sdf.rect` (never `sdf.box(..,0)`). Compact: 6px inner padding, tight line height. The lead icon is drawn in `draw_walk` into the reserved `icon_slot` gutter (mirrors `select_box.rs` drawing its caret after `view.draw_walk`).

```rust
//! `RefCardView`: one inspector reference card -- a compact, SQUARE-cornered
//! bordered row backing both MEMBERS and ASSOCIATIONS. Line 1 = element-kind
//! lead icon + name; line 2 (optional, dim) = a meta run (associations show the
//! direction glyph + role + multiplicity; members omit it). Pure-view here (a
//! `#[deref] View` hybrid mirroring `recent_row.rs`); interaction lands in the
//! navigation task. Values are pushed per row by the parent's FlatList loop.
//!
//! The border is drawn with `sdf.rect` for sharp square corners -- NEVER
//! `sdf.box(.., 0.0)`, which degenerates and floods in this fork. The lead glyph
//! is drawn in `draw_walk` over the reserved `icon_slot` gutter, the same
//! immediate-over-turtle idiom `select_box.rs` uses for its caret.

use crate::icons::{Icon, IconSet};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.RefCardViewBase = #(RefCardView::register_widget(vm))

    mod.widgets.RefCardView = set_type_default() do mod.widgets.RefCardViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 8.0, right: 8.0, top: 6.0, bottom: 6.0}
        spacing: 8.0
        show_bg: true

        // Square-cornered card: faint field-bg fill + low-alpha accent ring.
        // `sdf.rect` (NOT `sdf.box(..,0)`, which floods this fork).
        draw_bg +: {
            color: atlas.field_bg
            border: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0)
                sdf.fill_keep(vec4(self.color.x, self.color.y, self.color.z, 0.5))
                sdf.stroke(vec4(self.border.x, self.border.y, self.border.z, 0.20), 1.0)
                return sdf.result
            }
        }

        // Reserved leading gutter; the lead glyph is drawn over this rect in
        // draw_walk (an 18-unit icon centered in a 18-wide slot).
        icon_slot := View {
            width: 18.0
            height: 18.0
        }

        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: 1.0

            name := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 13
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.1
                    }
                }
            }
            meta := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.1
                    }
                }
            }
        }

        // Icon tint holder: an atlas-token DrawColor whose `.color` is copied
        // into `IconSet::draw` (no RGBA literal crosses Rust; see icons.rs).
        draw_icon: mod.draw.DrawColor{ color: atlas.text }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RefCardView {
    #[deref]
    view: View,
    #[live]
    icons: IconSet,
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    /// The lead glyph for this row's element kind; `None` draws no icon.
    #[rust]
    icon: Option<Icon>,
}

impl Widget for RefCardView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        // Draw the lead glyph over the reserved slot's drawn rect.
        if let Some(icon) = self.icon {
            let slot = self.view.view(cx, ids!(icon_slot)).area().rect(cx);
            let tint = self.draw_icon.color;
            self.icons.draw(cx, icon, slot, tint);
        }
        DrawStep::done()
    }
}

impl RefCardView {
    pub fn set_icon(&mut self, cx: &mut Cx, icon: Icon) {
        self.icon = Some(icon);
        self.view.redraw(cx);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.name)).set_text(cx, s);
    }
    /// Set line 2. An empty string hides the meta label (single-line card).
    pub fn set_meta(&mut self, cx: &mut Cx, s: &str) {
        self.view
            .widget(cx, ids!(textcol.meta))
            .set_visible(cx, !s.is_empty());
        self.view.label(cx, ids!(textcol.meta)).set_text(cx, s);
    }
}

impl RefCardViewRef {
    pub fn set_icon(&self, cx: &mut Cx, icon: Icon) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_icon(cx, icon);
        }
    }
    pub fn set_name(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_name(cx, s);
        }
    }
    pub fn set_meta(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_meta(cx, s);
        }
    }
}
```

- [ ] **Step 2: Register the module and the widget (ordering matters)**

In `crates\waml-editor\src\main.rs`, in the `mod` table (alphabetical block ~L32–33), add `mod ref_card;` and REMOVE `mod relationship_card;`:

```rust
mod recent_row;
mod ref_card;
mod scene;
```

(The line `mod relationship_card;` is deleted.)

In `crates\waml-editor\src\app.rs`, in `AppMain::script_mod`, replace the `relationship_card` registration (~L1739) with `ref_card`, keeping it BEFORE `inspector_panel::script_mod` (~L1740):

```rust
        crate::section_heading::script_mod(vm);
        crate::attr_row::script_mod(vm);
        // `RefCardView` must register before `inspector_panel`: the inspector's
        // MEMBERS and ASSOCIATIONS FlatLists mount it as a DSL child, and the DSL
        // resolves `mod.widgets.*` eagerly at `use`-time, not lazily. An
        // unregistered child is a dead, invisible node (finding survives both
        // green tests and review).
        crate::ref_card::script_mod(vm);
        crate::inspector_panel::script_mod(vm);
```

- [ ] **Step 3: Point the ASSOCIATIONS and MEMBERS lists at `RefCardView`**

In `crates\waml-editor\src\inspector_panel.rs`:

Remove the `RelationshipCardView` import (~L38) and add the `RefCardView` import + `Icon` is already imported:

```rust
use crate::ref_card::RefCardViewWidgetRefExt;
```
(Delete `use crate::relationship_card::RelationshipCardViewWidgetRefExt;`.)

In the DSL body, change the ASSOCIATIONS FlatList `Row` (~L204):

```rust
                rel_list := FlatList {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 8.0

                    Row := mod.widgets.RefCardView { }
                }
```

Replace the `members_lines := Label { ... }` block (~L179–191) with a MEMBERS FlatList wrapped in a gating `View` (mirrors `attr_list_wrap`/`rel_list_wrap`):

```rust
            members_heading := SectionHeading { }
            // Group members as compact reference cards (icon + name), one per row.
            members_list_wrap := View {
                width: Fill
                height: Fit
                flow: Down
                members_list := FlatList {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 8.0

                    Row := mod.widgets.RefCardView { }
                }
            }
```

- [ ] **Step 4: Add a per-kind card-icon helper**

In `crates\waml-editor\src\inspector_panel.rs`, add a pure helper near `meta_line` (~L450). (The `Group` arm maps to `PanelTop` for now; Task 4 refines it to `Icon::Group` once that glyph exists.)

```rust
/// The lead glyph for a reference card, by element kind. `Group` maps to a
/// placeholder until the dedicated `group` glyph lands (see the icon task).
fn ref_card_icon(kind: ElementKind) -> Icon {
    match kind {
        ElementKind::Edge => Icon::Spline,
        // Node / Group / Diagram / Placeholder: the class/panel glyph for now.
        _ => Icon::PanelTop,
    }
}
```

- [ ] **Step 5: Fill the MEMBERS list and repoint the ASSOCIATIONS fill in `draw_walk`**

In `fill_body_column` (~L906–922), replace the members text-set block with the FlatList wrapper gate (drop the `members_lines` label set entirely):

```rust
        // MEMBERS: reference cards (filled in the draw_walk list loop).
        let has_members = !view.members.is_empty();
        self.view
            .widget(cx, ids!(body.members_heading))
            .set_visible(cx, has_members);
        self.view
            .widget(cx, ids!(body.members_list_wrap))
            .set_visible(cx, has_members);
        if has_members {
            self.view
                .widget(cx, ids!(body.members_heading))
                .as_section_heading()
                .set_text(cx, "MEMBERS");
        }
```

In `draw_walk` (~L653–693), capture the members list uid alongside the others and push both card kinds. Replace the `rel_list` fill body and add a `members_list` branch:

```rust
        let attr_list_uid = self.view.widget(cx, ids!(body.attr_list)).widget_uid();
        let members_list_uid = self.view.widget(cx, ids!(body.members_list)).widget_uid();
        let rel_list_uid = self.view.widget(cx, ids!(body.rel_list)).widget_uid();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if !show_body {
                continue;
            }
            if item.widget_uid() == attr_list_uid {
                // (unchanged attribute fill)
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for (i, attr) in view.attributes.iter().enumerate() {
                            let item_id = attr_item_id(i, &attr.name);
                            let row = list.item(cx, item_id, id!(Row)).unwrap();
                            let rv = row.as_attr_row_view();
                            let (vis, name, ty, mult) = attr_line_parts(attr);
                            rv.set_visibility(cx, &vis);
                            rv.set_name(cx, &name);
                            rv.set_ty(cx, &ty);
                            rv.set_mult(cx, &mult);
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
            if item.widget_uid() == members_list_uid {
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for (i, m) in view.members.iter().enumerate() {
                            let item_id = member_item_id(i, &m.key);
                            let row = list.item(cx, item_id, id!(Row)).unwrap();
                            let rv = row.as_ref_card_view();
                            rv.set_icon(cx, ref_card_icon(m.kind));
                            rv.set_name(cx, &m.label);
                            rv.set_meta(cx, ""); // members are single-line
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
            if item.widget_uid() == rel_list_uid {
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for (i, assoc) in view.associations.iter().enumerate() {
                            let item_id = LiveId::from_str(&format!(
                                "{}-{}-{}",
                                i, assoc.kind, assoc.other_label
                            ));
                            let row = list.item(cx, item_id, id!(Row)).unwrap();
                            let rv = row.as_ref_card_view();
                            rv.set_icon(cx, ref_card_icon(assoc.target_kind));
                            rv.set_name(cx, &assoc.other_label);
                            // Line 2: direction glyph + kind/role/multiplicity run.
                            rv.set_meta(cx, &format!("{} {}", dir_glyph(assoc.dir), meta_line(assoc)));
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
        }
```

- [ ] **Step 6: Add the `member_item_id` helper + its tests**

Near `attr_item_id` (~L474) in `inspector_panel.rs`:

```rust
/// `FlatList` item id for member row `i` (keyed `key`). Index-prefixed so two
/// members sharing a key still key to distinct list items (mirrors
/// `attr_item_id`).
fn member_item_id(i: usize, key: &str) -> LiveId {
    LiveId::from_str(&format!("{i}-{key}"))
}
```

Add unit tests in `inspector_panel.rs` `mod tests`:

```rust
    #[test]
    fn member_item_id_distinguishes_duplicate_keys() {
        assert_ne!(member_item_id(0, "k"), member_item_id(1, "k"));
    }

    #[test]
    fn member_item_id_is_stable_for_same_index_and_key() {
        assert_eq!(member_item_id(2, "k"), member_item_id(2, "k"));
    }

    #[test]
    fn ref_card_icon_maps_edge_and_node() {
        assert!(matches!(ref_card_icon(ElementKind::Edge), Icon::Spline));
        assert!(matches!(ref_card_icon(ElementKind::Node), Icon::PanelTop));
    }
```

- [ ] **Step 7: Delete `relationship_card.rs`**

```bash
git rm crates/waml-editor/src/relationship_card.rs
```

Confirm no dangling references: `rg -n "relationship_card|RelationshipCardView|as_relationship_card_view" crates/waml-editor/src` returns nothing.

- [ ] **Step 8: Run the gate**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean. (If clippy flags `RefCardView::set_meta`/`set_icon` etc. as never-read, that means a fill site was missed — re-check Step 5.)

- [ ] **Step 9: Visual verify (pid-safe)**

Build and launch ONLY the worktree's own exe and screenshot by PID (never by process name):

```powershell
# From the worktree root. Launch the worktree's OWN build, capture its PID.
$exe = "C:\dev\waml\.worktrees\inspector-nav-cards\target\debug\waml-editor.exe"
cargo build -p waml-editor
$p = Start-Process $exe -PassThru
Start-Sleep -Seconds 6
# capture ONLY $p.Id via your screenshot-by-pid helper, then:
Stop-Process -Id $p.Id -Confirm:$false
```

Open the Orders diagram, pick the "Connectors" (or "Sales") Group in the inspector picker: MEMBERS render as compact SQUARE-cornered cards with a lead icon + name; ASSOCIATIONS render as the same card style (icon + name + dim direction/role/mult meta). No rounded corners; no `RelationshipCardView` remnants; ATTRIBUTES unchanged vs `scratchpad/typescale-08.png`.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/ref_card.rs crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git rm crates/waml-editor/src/relationship_card.rs
git commit -m "feat(inspector): shared RefCardView for members + associations; drop RelationshipCardView"
```

---

### Task 3: Navigation — click a card to repoint the inspector and select the node

**Files:**
- Modify: `crates\waml-editor\src\ref_card.rs` (interaction: hover cursor + click; carry `(key, kind)`; `nav_target` reader)
- Modify: `crates\waml-editor\src\inspector.rs` (pure `subject_from` helper + test)
- Modify: `crates\waml-editor\src\inspector_panel.rs` (set each card's target during fill; `Inspector::navigate` reader)
- Modify: `crates\waml-editor\src\canvas.rs` (`GraphCanvas::select_by_key`)
- Modify: `crates\waml-editor\src\class_diagram_view.rs` (consume `inspector.navigate` → repoint + canvas select)

**Interfaces:**
- Produces:
  - `pub fn subject_from(key: &str, kind: ElementKind) -> Option<Subject>` in `inspector.rs` (Node→`Classifier`, Group→`Group`, Edge→`Edge`, Diagram/Placeholder→`None`).
  - `RefCardView::set_target(&mut self, key: &str, kind: ElementKind)` + `RefCardView::nav_target(&self, actions: &Actions) -> Option<(String, ElementKind)>` (+ `RefCardViewRef` mirrors).
  - `Inspector::navigate(&mut self, cx: &mut Cx, actions: &Actions) -> Option<(String, ElementKind)>`.
  - `GraphCanvas::select_by_key(&mut self, cx: &mut Cx, key: &str)`.
- Consumes: Task 1's `ElementRef`/`AssocRow.target_key`/`target_kind`; Task 2's `RefCardView` + fill sites.

- [ ] **Step 1: Write the failing test for `subject_from`**

In `crates\waml-editor\src\inspector.rs` `mod tests`:

```rust
    #[test]
    fn subject_from_maps_each_kind() {
        assert_eq!(
            subject_from("k", ElementKind::Node),
            Some(Subject::Classifier("k".into()))
        );
        assert_eq!(
            subject_from("g", ElementKind::Group),
            Some(Subject::Group("g".into()))
        );
        assert_eq!(
            subject_from("a->b", ElementKind::Edge),
            Some(Subject::Edge("a->b".into()))
        );
        assert_eq!(subject_from("d", ElementKind::Diagram), None);
        assert_eq!(subject_from("", ElementKind::Placeholder), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p waml-editor --lib subject_from_maps_each_kind 2>&1 | rg -n "cannot find|error\[|FAILED"`
Expected: FAIL — `subject_from` is not defined.

- [ ] **Step 3: Implement `subject_from`**

In `crates\waml-editor\src\inspector.rs`, after `subject_to_index` (~L245):

```rust
/// Build the `Subject` a navigable reference points at. Node/Group/Edge map to
/// their inspectable subjects; Diagram/Placeholder are not inspectable (`None`).
pub fn subject_from(key: &str, kind: ElementKind) -> Option<Subject> {
    match kind {
        ElementKind::Node => Some(Subject::Classifier(key.to_string())),
        ElementKind::Group => Some(Subject::Group(key.to_string())),
        ElementKind::Edge => Some(Subject::Edge(key.to_string())),
        ElementKind::Diagram | ElementKind::Placeholder => None,
    }
}
```

Run: `cargo test -p waml-editor --lib subject_from_maps_each_kind` → PASS.

- [ ] **Step 4: Make `RefCardView` clickable and carry a target**

In `crates\waml-editor\src\ref_card.rs`, import `ElementKind`, add the fields, a `RefCardViewAction`, `handle_event` interaction, and the reader. Update the doc header's "Pure-view here" line since it now handles clicks.

Add to imports:

```rust
use crate::inspector::ElementKind;
```

Add the action enum (above the struct):

```rust
/// Emitted (grouped through the parent FlatList) when a card is clicked. The
/// parent reads it via `items_with_actions` + `RefCardViewRef::nav_target`.
#[derive(Clone, Debug, Default)]
pub enum RefCardViewAction {
    #[default]
    None,
    Clicked,
}
```

Extend the struct with target + hover state:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct RefCardView {
    #[deref]
    view: View,
    #[live]
    icons: IconSet,
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    #[rust]
    icon: Option<Icon>,
    /// The navigate target this row points at (set per draw by the parent).
    #[rust]
    nav_key: String,
    #[rust]
    nav_kind: Option<ElementKind>,
    /// Pointer-over, self-managed from FingerHoverIn/Out (drives the cursor).
    #[rust]
    hovered: bool,
}
```

Replace `handle_event` with the interactive version:

```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, RefCardViewAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                self.hovered = true;
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = false;
            }
            _ => {}
        }
    }
```

Add the target setter + reader to `impl RefCardView`:

```rust
    pub fn set_target(&mut self, key: &str, kind: ElementKind) {
        self.nav_key = key.to_string();
        self.nav_kind = Some(kind);
    }
    /// `Some((key, kind))` when this row emitted a click in `actions`.
    pub fn nav_target(&self, actions: &Actions) -> Option<(String, ElementKind)> {
        let clicked = actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RefCardViewAction::Clicked));
        if clicked {
            self.nav_kind.map(|k| (self.nav_key.clone(), k))
        } else {
            None
        }
    }
```

Add to `impl RefCardViewRef`:

```rust
    pub fn set_target(&self, key: &str, kind: ElementKind) {
        if let Some(mut i) = self.borrow_mut() {
            i.set_target(key, kind);
        }
    }
    pub fn nav_target(&self, actions: &Actions) -> Option<(String, ElementKind)> {
        self.borrow().and_then(|i| i.nav_target(actions))
    }
```

- [ ] **Step 5: Set each card's target during the fill loops**

In `crates\waml-editor\src\inspector_panel.rs` `draw_walk`, in the `members_list` branch (after `rv.set_name`/`set_meta`):

```rust
                            rv.set_target(&m.key, m.kind);
```

In the `rel_list` branch (after `rv.set_name`/`set_meta`):

```rust
                            rv.set_target(&assoc.target_key, assoc.target_kind);
```

- [ ] **Step 6: Add `Inspector::navigate`**

In `crates\waml-editor\src\inspector_panel.rs`, in `impl Inspector` (near `take_open_request`, ~L1201), add:

```rust
    /// A member/association card was clicked this pass. Scans both section
    /// FlatLists' grouped actions and returns the clicked row's `(key, kind)`.
    /// `App`/`ClassDiagramView` repoints the inspector and selects the node.
    pub fn navigate(&mut self, cx: &mut Cx, actions: &Actions) -> Option<(String, ElementKind)> {
        for list_id in [ids!(body.members_list), ids!(body.rel_list)] {
            let list = self.view.flat_list(cx, list_id);
            for (_item_id, item) in list.items_with_actions(actions) {
                if let Some(t) = item.as_ref_card_view().nav_target(actions) {
                    return Some(t);
                }
            }
        }
        None
    }
```

> If `ids!(...)` cannot be used as an array element directly, hoist to two explicit `self.view.flat_list(cx, ids!(body.members_list))` / `ids!(body.rel_list)` blocks; the loop is a convenience, not load-bearing.

- [ ] **Step 7: Add `GraphCanvas::select_by_key`**

In `crates\waml-editor\src\canvas.rs`, next to `update_scene` (~L1977), add:

```rust
    /// Select the node whose key is `key` (inspector-driven navigation). Sets
    /// `selected_key` and re-resolves `selected` by key against the current
    /// scene; a key with no node in this scene (e.g. an edge) clears the
    /// selection but is otherwise a no-op. Repaints the highlight.
    pub fn select_by_key(&mut self, cx: &mut Cx, key: &str) {
        self.selected_key = Some(key.to_string());
        self.selected = selection_index(&self.scene.nodes, Some(key));
        if self.selected.is_none() {
            self.selected_key = None;
        }
        self.draw_bg.redraw(cx);
    }
```

(`selection_index` is the existing pure fn at ~L634, already unit-tested at ~L2448.)

- [ ] **Step 8: Wire navigation in `ClassDiagramView::handle`**

In `crates\waml-editor\src\class_diagram_view.rs`, add the `subject_from` import:

```rust
use crate::inspector::{diagram_elements, subject_from, Subject};
```

In `DocView::handle`, add a navigation block BEFORE the canvas-action match (after the `take_open_request` block, ~L143):

```rust
        // Reference-card navigation: a member/association card was clicked.
        // Repoint the inspector AND select the node on the canvas (edge keys
        // repoint only -- no node to select).
        if let Some((key, kind)) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|mut inspector| inspector.navigate(cx, actions))
        {
            if let Some(subject) = subject_from(&key, kind) {
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, subject);
                }
                if let Some(mut canvas) =
                    body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.select_by_key(cx, &key);
                }
            }
            return out;
        }
```

> The two separate `borrow_mut` scopes for the inspector are deliberate — `navigate` needs `&mut inspector` and must be dropped before the second borrow. Do not hold both the inspector and canvas borrows at once.

- [ ] **Step 9: Run the gate**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean, including `subject_from_maps_each_kind` and the existing `selection_index` test.

- [ ] **Step 10: Visual verify (pid-safe)**

Launch the worktree exe by PID (per Task 2 Step 9). Open the Orders diagram, select a Group: click a MEMBER card → the inspector repoints to that node AND the node highlights on the canvas. Click an ASSOCIATION card whose target is a node → same. Confirm the Hand cursor over cards. Stop only the spawned PID.

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/ref_card.rs crates/waml-editor/src/inspector.rs crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/canvas.rs crates/waml-editor/src/class_diagram_view.rs
git commit -m "feat(inspector): navigable ref cards repoint the inspector and select the node"
```

---

### Task 4: New Lucide `group` glyph in the icon catalog

**Files:**
- Create: `crates\waml-editor\resources\icons\group.svg` (source of record for the glyph)
- Modify: `crates\waml-editor\src\icons.rs` (shader + IconSet field + struct field + `get` arm + enum + `ALL` + `label` + count assertions)
- Modify: `crates\waml-editor\src\inspector_panel.rs` (map `ElementKind::Group` → `Icon::Group` at the SelectBox lead + in `ref_card_icon`)

**Interfaces:**
- Produces: `Icon::Group` (catalog entry #90, appended after `Icon::Search`); `Icon::Group.label() == "group"`; `IconSet.group` field.
- Consumes: nothing from later tasks.

> The catalog invariant is load-bearing: touch ALL of {DSL shader, DSL instance field, struct field, `get` arm, enum variant, `ALL` array (and its `[Icon; N]` length), `label` arm} in the SAME ORDER, and bump the three `90`-count assertions to `91`. Append `Group` LAST (after `Search`) so existing `ALL[..]` index assertions at the edges stay valid.

- [ ] **Step 1: Write the failing count test bump**

In `crates\waml-editor\src\icons.rs` `mod tests`, update `icon_all_has_90_entries` (rename + bump) and the `seen.len()` assertion, and add a tail-order assertion:

```rust
    #[test]
    fn icon_all_has_91_entries() {
        assert_eq!(Icon::ALL.len(), 91);
    }
```
```rust
        assert_eq!(seen.len(), 91);
```

Add to `icon_all_is_in_field_order_at_the_edges`:

```rust
        assert_eq!(Icon::ALL[89], Icon::Search);
        assert_eq!(Icon::ALL[90], Icon::Group);
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p waml-editor --lib icons:: 2>&1 | rg -n "cannot find|mismatched|error\[|FAILED"`
Expected: FAIL — `Icon::Group` undefined and `ALL.len()` is still 90.

- [ ] **Step 3: Add the source SVG**

Create `crates\waml-editor\resources\icons\group.svg` with the Lucide `group` path (four corner brackets framing two overlapping rounded rects):

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 7V5c0-1.1.9-2 2-2h2"/>
  <path d="M17 3h2c1.1 0 2 .9 2 2v2"/>
  <path d="M21 17v2c0 1.1-.9 2-2 2h-2"/>
  <path d="M7 21H5c-1.1 0-2-.9-2-2v-2"/>
  <rect width="7" height="5" x="7" y="7" rx="1"/>
  <rect width="7" height="5" x="10" y="12" rx="1"/>
</svg>
```

- [ ] **Step 4: Add the `mod.draw.IconGroup` shader**

In `crates\waml-editor\src\icons.rs`, inside the `script_mod!` block, add the shader (hand-authored SDF in the shader's local `rect_size`, normalized 0..1 * `s`; brackets as stroked L-paths, the two grouped items as small rounded `sdf.box` with a real radius). Place it right before `mod.draw.IconSquareMenu` (keeping rough alphabetical order is fine; ORDER OF THE STRUCT/ENUM/ALL is what is load-bearing, not shader-block position, but append its instance/field/arm LAST):

```rust
    // Group: four corner brackets framing two overlapping rounded rectangles.
    // Faithful port of resources/icons/group.svg (hand-tuned for the HUD size;
    // silhouette is a first pass, tuned live in `icon_harness`).
    mod.draw.IconGroup = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Top-left bracket.
            sdf.move_to(s * 0.1250, s * 0.2917)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.line_to(s * 0.2917, s * 0.1250)
            sdf.stroke(self.color, w)
            // Top-right bracket.
            sdf.move_to(s * 0.7083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.2917)
            sdf.stroke(self.color, w)
            // Bottom-right bracket.
            sdf.move_to(s * 0.8750, s * 0.7083)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.7083, s * 0.8750)
            sdf.stroke(self.color, w)
            // Bottom-left bracket.
            sdf.move_to(s * 0.2917, s * 0.8750)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.7083)
            sdf.stroke(self.color, w)
            // Upper grouped rect (rounded; real radius so it is not degenerate).
            sdf.box(s * 0.2917, s * 0.2917, s * 0.2917, s * 0.2083, s * 0.0625)
            sdf.stroke(self.color, w)
            // Lower grouped rect, offset to overlap.
            sdf.box(s * 0.4167, s * 0.5000, s * 0.2917, s * 0.2083, s * 0.0625)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }
```

- [ ] **Step 5: Wire the catalog invariant (append `group` LAST everywhere)**

In `crates\waml-editor\src\icons.rs`:

DSL `IconSet` instance (after `search: mod.draw.IconSearch{...}`, ~L3171):
```rust
        group: mod.draw.IconGroup{ color: atlas.accent }
```

Struct `IconSet` field (after `pub search: DrawColor,`, ~L3358):
```rust
    #[live]
    pub group: DrawColor,
```

`get` match arm (after `Icon::Search => &mut self.search,`, ~L3459):
```rust
            Icon::Group => &mut self.group,
```

Enum variant (after `Search,`, ~L3567):
```rust
    Group,
```

`ALL` array: bump the length and append the entry:
```rust
    pub const ALL: [Icon; 91] = [
```
```rust
        Icon::Search,
        Icon::Group,
    ];
```

`label` arm (after `Icon::Search => "search",`, ~L3760):
```rust
            Icon::Group => "group",
```

- [ ] **Step 6: Map `ElementKind::Group` → `Icon::Group` in the inspector**

In `crates\waml-editor\src\inspector_panel.rs`, in `build_select_items`, change the `ElementKind::Group` arm (~L1045–1051) to lead with the new glyph:

```rust
                ElementKind::Group => (
                    // The Lucide group glyph -- distinct from the diagram's solid
                    // `Frame` and any node's catalog icon. Drives the collapsed
                    // select-box lead (the panel header) too.
                    SelectLead::Icon(Icon::Group),
                    row.label.clone(),
                    true,
                ),
```

Refine `ref_card_icon` so groups (should any group-kind ref surface in a card) use the real glyph:

```rust
fn ref_card_icon(kind: ElementKind) -> Icon {
    match kind {
        ElementKind::Group => Icon::Group,
        ElementKind::Edge => Icon::Spline,
        _ => Icon::PanelTop,
    }
}
```

- [ ] **Step 7: Run to verify tests pass**

Run: `cargo test -p waml-editor --lib icons::`
Expected: PASS — `icon_all_has_91_entries`, `icon_labels_are_unique_and_nonempty` (91 unique), edge-order incl. `ALL[90] == Group`.

- [ ] **Step 8: Run the gate**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean. (`SquareDashedTopSolid` stays in the catalog and is now unused by `ElementKind::Group` — that is fine; catalog glyphs are deliberately kept even when unused.)

- [ ] **Step 9: Visual verify (pid-safe, incl. the icon harness)**

Build/launch the worktree exe by PID. In the main app, the Group's collapsed select-box lead + the header show the new `group` glyph (two overlapping brackets/rects), not the dashed box. Optionally run the icon proof grid to eyeball the silhouette and tune it live:

```powershell
cargo run -p waml-editor --bin icon_harness
```
Confirm `group` reads as a group; if the silhouette is off, tune the SDF coordinates in the shader (first-pass tuning is expected per the catalog convention). Stop only the spawned PID(s).

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/resources/icons/group.svg crates/waml-editor/src/icons.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(icons): add Lucide group glyph; use it for ElementKind::Group"
```

---

### Task 5: Tighten section spacing + nudge the collapsed select-box baseline

**Files:**
- Modify: `crates\waml-editor\src\inspector_panel.rs` (body column `spacing`, card padding/line-height constants)
- Modify: `crates\waml-editor\src\select_box.rs` (selected-label baseline `cy - 8.0` → `cy - 9.5`)

**Interfaces:**
- Consumes: Task 2's `RefCardView` DSL (padding/spacing), Task 2's `member/rel` FlatList `spacing`.
- Produces: nothing consumed downstream (pure visual tuning).

> These are numeric/tuning tweaks. There is no unit test for pixel spacing — the acceptance is the pid-safe visual verify. Keep changes small and reversible; tune against the live panel.

- [ ] **Step 1: Tighten the body column + inter-card gaps**

In `crates\waml-editor\src\inspector_panel.rs`, reduce the `body` Turtle column's `spacing` (~L115) from `16.0`:

```rust
        body := View {
            width: Fill
            height: Fit
            flow: Down
            visible: false
            padding: Inset{left: 16.0, right: 16.0, top: 0.0, bottom: 16.0}
            spacing: 12.0
```

Reduce the ASSOCIATIONS and MEMBERS FlatList `spacing` from `8.0` to `6.0` (both `rel_list` and `members_list`, matching `attr_list`'s tighter feel):

```rust
                    spacing: 6.0
```

- [ ] **Step 2: Tighten the `RefCardView` inner padding/line-height (if still loose)**

In `crates\waml-editor\src\ref_card.rs`, if the cards still read tall in Step 4's verify, reduce the root `padding` top/bottom from `6.0` to `5.0` and confirm `textcol.spacing` is `1.0`. (Leave as-is if already tight enough — do not over-compress single-line member cards.)

- [ ] **Step 3: Nudge the collapsed select-box baseline**

In `crates\waml-editor\src\select_box.rs`, in `draw_walk`, change the selected label's y from `cy - 8.0` to `cy - 9.5` so the 14px cap height centers in the 32px box (~L174–175):

```rust
            self.draw_label
                .draw_abs(cx, dvec2(label_x, cy - 9.5), &sel.label);
```

- [ ] **Step 4: Run the gate**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean (no behavioral tests affected).

- [ ] **Step 5: Visual verify (pid-safe)**

Build/launch the worktree exe by PID. Confirm: section gaps and inter-card gaps read tighter (not cramped); the collapsed select-box selected name is vertically centered (no longer sitting low); ATTRIBUTES still match `scratchpad/typescale-08.png`. Compare the Group "Connectors" panel before/after. Stop only the spawned PID.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/ref_card.rs crates/waml-editor/src/select_box.rs
git commit -m "style(inspector): tighten section spacing and center the collapsed select-box label"
```

---

## Self-Review

**Spec coverage:**
- Component 1 (`RefCardView` replacing `RelationshipCardView`, square `sdf.rect` border, icon+name+optional meta, navigate on click) → Tasks 2 (widget + list switch + removal) + 3 (interaction/navigate). ✅
- Component 2 (read model: `ElementRef`, `members: Vec<ElementRef>`, `AssocRow.target_key`/`target_kind`, populated in builders) → Task 1. ✅
- Component 3 (navigation wiring: `Inspector::navigate`, `Canvas::select_by_key`, `class_diagram_view` repoint + select, edges repoint-only) → Task 3. ✅
- Component 4 (body spacing) → Task 5. ✅
- Component 5 (new Lucide `group` glyph, full catalog invariant + count bumps, map `ElementKind::Group`) → Task 4. ✅
- Component 6 (collapsed select-box baseline nudge) → Task 5. ✅
- Testing expectations (build_view populates members as ElementRefs; AssocRow target resolves to far endpoint; edge-vs-node kind; pure helpers `meta_line`/`subject_from`/`member_item_id`/`ref_card_icon` unit-tested; pid-safe visual verify per task) → Tasks 1, 3, 4 tests + per-task visual steps. ✅
- Out-of-scope respected: the `!show_picker` immediate-mode body is only touched to fix the `ElementRef` read type (unavoidable compile fix), not restructured; web/Svelte untouched; picker popup/list behavior unchanged beyond the group-icon swap + baseline nudge. ✅

**Type consistency:** `ElementRef {key,kind,label}`, `AssocRow.target_key/target_kind`, `subject_from`, `RefCardView::{set_icon,set_name,set_meta,set_target,nav_target}`, `Inspector::navigate`, `GraphCanvas::select_by_key`, `ref_card_icon`, `member_item_id` — names are used identically across the tasks that define and consume them.

**Green-per-unit:** Task 1's new fields are read by derives + tests (no dead_code). Task 2's `RefCardView` is pure-view and fully consumed by both fill loops (no dead methods). Task 3 lands interaction + `Canvas::select_by_key` + its sole caller together. Task 4 appends the glyph and its consumer (`ElementKind::Group` map) in the same unit. Task 5 is pure tuning. Each task ends green on `fmt` + `clippy -D warnings` + `test`.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-07-24-inspector-nav-cards.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** - execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
