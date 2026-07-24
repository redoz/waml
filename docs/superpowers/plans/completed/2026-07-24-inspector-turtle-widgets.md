# Inspector Turtle Child Widgets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inspector panel's immediate-mode body (attributes / relationships / description) with reusable Turtle-laid-out child widgets so rows self-size, spacing is declarative, and the card/row/heading shapes become reusable.

**Architecture:** The `Inspector` container stays a `#[deref] View` widget. For the diagram/picker view (`show_picker == true`) the body becomes a declared Turtle `flow:Down` column of child widgets, drawn by `self.view.draw_walk` with the `start_screen.rs` FlatList-interpose idiom for the two variable-length sections. Three new pure-view widgets (`SectionHeading`, `AttrRowView`, `RelationshipCardView`) mirror `recent_row.rs` exactly. The single-item tree-preview path (`show_picker == false`) keeps its existing immediate-mode body untouched. `inspector.rs` (the `InspectorView` projection) is the data source and is never modified.

**Tech Stack:** Rust, Makepad (redoz fork), the crate's `script_mod!` DSL, `#[derive(Script, ScriptHook, Widget)]` + `#[deref] View` hybrid widgets, `FlatList`.

## Global Constraints

Every task's requirements implicitly include this section.

- **Worktree only.** All edits happen in `C:\dev\waml\.worktrees\inspector-typescale`. Verify `git rev-parse --show-toplevel` resolves to that path before editing. NEVER edit the main `C:\dev\waml` checkout.
- **Baseline.** Build from the committed baseline (`origin/main` / `HEAD` = `358e87b`), where the inspector body is immediate-mode with the OLD type scale. There is an uncommitted exploratory `git diff` in `inspector_panel.rs` + `select_box.rs` — treat it as a **value reference only** (type-scale numbers, divider, bold name). Do not depend on those uncommitted edits being present; every value this plan needs is written out below.
- **Scope: `show_picker == true` (diagram/picker) view ONLY.** The `show_picker == false` single-item tree-preview path (used by `classifier_preview_view.rs` / `source_view.rs`, top overlaps the bar — a known unfinished path) is OUT of scope and must render exactly as it does today. Native (Makepad) only; the web/Svelte frontend is untouched. `inspector.rs` is untouched.
- **Type scale (port verbatim into the new widgets / column Labels):**
  - section heading: IBM Plex Sans SemiBold `font_size: 10`, `atlas.text_dim`
  - kind line: IBM Plex Sans Medium `font_size: 11`, `atlas.accent` (plain `Label` in the column)
  - attr name: IBM Plex **Mono** Regular `font_size: 11`, `atlas.text`
  - attr type: IBM Plex Mono Regular `font_size: 11`, `atlas.accent`
  - attr `[mult]`: IBM Plex Mono Regular `font_size: 11`, `atlas.text_dim`
  - relationship name: IBM Plex Sans SemiBold `font_size: 12`, `atlas.text`
  - relationship glyph: IBM Plex Sans Regular `font_size: 13`, `atlas.accent`
  - relationship meta: IBM Plex Sans Regular `font_size: 11`, `atlas.text_dim`
  - selectbox name: IBM Plex Sans Bold `font_size: 14`, `atlas.text`
  - divider: `atlas.surface_border`, 1px hairline
- **Gate after EVERY task** (run from the worktree root):
  `cargo fmt` then `cargo clippy --workspace --all-targets -- -D warnings` then `cargo test --workspace`.
  `-D warnings` promotes rustc `dead_code` to a hard error: any struct field / const / fn / DSL pen a task leaves with no consumer FAILS the gate. Every symbol a task adds must be wired to a consumer within that same task; every symbol a task orphans must be deleted within that same task.
- **Dead-node trap / registration order.** A custom widget mounted as a DSL child is a dead, invisible, unqueryable node unless its module's `script_mod(vm)` is registered in `app.rs` BEFORE the consuming module's — the DSL resolves `mod.widgets.*` eagerly at `use`-time. All three new widgets are consumed by `inspector_panel`, so each must register between `crate::select_box::script_mod(vm)` (`app.rs:1598`) and `crate::inspector_panel::script_mod(vm)` (`app.rs:1599`), and be added to `main.rs`'s `mod` list. The registering happens in the same task that creates and consumes the widget.
- **Live-reload caveat.** Only `live_design!`/DSL value overrides hot-reload; new struct fields, consts, and Rust logic need a full rebuild via `run-native.ps1` (which builds its own `$PSScriptRoot` dir — launch the worktree's own copy).
- **Screenshot/verify by SPECIFIC pid only.** Never capture/kill by process name (kills the user's own running editor). Makepad ignores synthetic `PostMessage` clicks — use `realclick-pid.ps1`. A fresh launch does not auto-select a node; click a canvas node in the Orders diagram first. Visual-verification is folded into each code task; there is NEVER a standalone verify-only task (a no-code-diff task stalls the `Plan-Tasks:` trailer tracking).

---

## File Structure

- `crates/waml-editor/src/select_box.rs` — MODIFY. Drop the now-dead `draw_frame` field + its `AccentFrame` DSL; bold-14 flat header. (Task 1)
- `crates/waml-editor/src/section_heading.rs` — CREATE. `SectionHeading` widget: one eyebrow label. (Task 2)
- `crates/waml-editor/src/attr_row.rs` — CREATE. `AttrRowView` widget: `flow:Right` separate labels. (Task 3)
- `crates/waml-editor/src/relationship_card.rs` — CREATE. `RelationshipCardView` widget: bordered `flow:Down` card. (Task 4)
- `crates/waml-editor/src/inspector_panel.rs` — MODIFY across Tasks 2–4. DSL gains a `body` column; `draw_walk` gains the `show_picker` declarative branch (interpose idiom); the `show_picker == false` branch keeps the baseline immediate-mode body verbatim.
- `crates/waml-editor/src/app.rs` — MODIFY across Tasks 2–4. Register each new widget in dependency order.
- `crates/waml-editor/src/main.rs` — MODIFY across Tasks 2–4. Add `mod` declarations.

The three widgets mirror `recent_row.rs`: a `#[derive(Script, ScriptHook, Widget)]` struct with `#[deref] view: View`, a `script_mod!` DSL block declaring the Turtle layout, granular per-line setters on both the struct and the generated `…Ref`, and a trivial `Widget` impl delegating `handle_event`/`draw_walk` to `self.view` (these three are read-only — no hover/click, unlike `RecentRowView`).

---

### Task 1: SelectBox flat-header fix

Make the `SelectBox`'s flat web-header look intentional: keep the bold-14 selected-name, and delete the `draw_frame` field + its `AccentFrame` DSL, which the flat draw no longer paints. Keep the `draw_active` open-state accent ring (drawn only while the list is open). This is self-contained and does not touch the inspector body.

**Files:**
- Modify: `crates/waml-editor/src/select_box.rs`

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: nothing other tasks rely on (`SelectBox`'s public API — `set_items`/`set_selected`/`open_request`/`on_closed`/`is_open` — is unchanged).

Note: `select_box.rs` carries `#![allow(dead_code)]`, so an orphaned field would not fail the gate — but the spec requires the removal so no reader thinks a frame still paints.

- [ ] **Step 1: Bold-14 flat header in the DSL**

In the `script_mod!` block, delete the `draw_frame` DSL line (the `AccentFrame` field material) and set the label to bold-14. The `draw_label` block must read exactly:

```
        // Web-header style: the selected subject's name reads as a bold title,
        // not a small combo-field label (mirrors the web inspector header).
        draw_label +: {
            color: atlas.text
            text_style: theme.font_bold{ font_size: 14 line_spacing: 1.2 }
        }
```

Delete this line from the DSL block (the field material for the boxed frame):

```
        // Field material: the shared Atlas frame + field-bg fill.
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
```

Leave `draw_active`, `draw_badge`, `draw_badge_text`, `draw_icon_idle`, `draw_caret` exactly as they are.

- [ ] **Step 2: Remove the `draw_frame` struct field**

In `pub struct SelectBox`, delete these three lines:

```rust
    #[redraw]
    #[live]
    draw_frame: DrawColor,
```

- [ ] **Step 3: Make `draw_walk` flat (stop drawing the frame)**

In `Widget for SelectBox::draw_walk`, replace the card-frame draw:

```rust
        // Card.
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, rect);
```

with the flat-look comment (no draw):

```rust
        // Flat web-header look: no boxed field frame -- the leading kind icon,
        // bold name, and trailing caret carry the affordance over the bare panel.
        // (The open-state accent ring below still draws when the list is open.)
```

Also nudge the selected-icon rect and label baseline to the bold-14 metrics — in the `SelectLead::Icon` arm change the icon rect to `pos: dvec2(rect.pos.x + 8.0, cy - 9.0), size: dvec2(18.0, 18.0)`, and change the trailing `self.draw_label.draw_abs(cx, dvec2(label_x, cy - 6.0), &sel.label);` to `cy - 8.0`. Leave the `draw_active` open-ring block (`if self.open { self.draw_active.draw_abs(cx, rect); }`) untouched.

- [ ] **Step 4: Run the gate**

Run: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS. The three existing `decide_closed` unit tests still pass (logic unchanged).

- [ ] **Step 5: Visual-verify (folded into this task, native, pid-safe)**

Build and launch the worktree's own native app via `run-native.ps1`; capture-by-pid with `realclick-pid.ps1` after clicking a canvas node in the Orders diagram. Confirm: the selectbox shows the subject name in a bold-14 title with NO boxed field frame around it, the leading kind icon and trailing caret are present, and opening the list still draws the accent ring. (The accent edge still visible around the panel is the Inspector panel's own `AccentFrame`, `inspector_panel.rs` `draw_bg` — intended, stays.)

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/select_box.rs
git commit -m "refactor(select-box): drop dead draw_frame, intentional flat bold-14 header"
```

---

### Task 2: SectionHeading widget + declarative body column (structural switch)

Create the `SectionHeading` widget and switch the `show_picker` body from immediate-mode to a declared Turtle `flow:Down` column drawn by `self.view.draw_walk`. This task establishes the column with: divider, kind line, stereotype chips, the three `SectionHeading`s, and the editable description — plus **interim** plain `Label`s (`attr_lines`, `rel_lines`) that keep attributes/relationships visible as stacked text. Tasks 3 and 4 replace those two interim Labels with `FlatList`s of real row widgets. The `show_picker == false` branch keeps the baseline immediate-mode body verbatim.

**Files:**
- Create: `crates/waml-editor/src/section_heading.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod section_heading;`)
- Modify: `crates/waml-editor/src/app.rs` (register between `:1598` and `:1599`)
- Modify: `crates/waml-editor/src/inspector_panel.rs` (DSL `body` column; `draw_walk` rewrite; remove the now-orphaned `draw_divider` pen)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces (used by Tasks 3–4):
  - Widget `SectionHeading` with `pub fn set_text(&mut self, cx: &mut Cx, s: &str)`, and `SectionHeadingRef::set_text(&self, cx: &mut Cx, s: &str)`; generated accessor `WidgetRef::as_section_heading()`.
  - DSL node ids under the container: `body` (the column `View`), `body.divider`, `body.kind`, `body.stereo`, `body.attr_heading`, `body.attr_lines`, `body.rel_heading`, `body.rel_lines`, `body.desc_heading`, `body.desc`.
  - Pure helper `attr_line_parts(&AttrRow) -> (String, String, String, String)` returning `(visibility, name, ty, mult)` display strings — reused by Task 3.

- [ ] **Step 1: Write the failing test for the attribute-parts formatter**

Add to the `#[cfg(test)] mod tests` in `inspector_panel.rs`:

```rust
    #[test]
    fn attr_line_parts_formats_visibility_and_multiplicity() {
        let a = crate::inspector::AttrRow {
            name: "items".into(),
            ty: "Product".into(),
            multiplicity: "0..*".into(),
            visibility: "+".into(),
        };
        assert_eq!(
            attr_line_parts(&a),
            ("+ ".into(), "items".into(), "Product".into(), "  [0..*]".into())
        );
    }

    #[test]
    fn attr_line_parts_elides_empty_visibility_and_trivial_multiplicity() {
        let a = crate::inspector::AttrRow {
            name: "id".into(),
            ty: "Uuid".into(),
            multiplicity: "1".into(),
            visibility: String::new(),
        };
        assert_eq!(
            attr_line_parts(&a),
            (String::new(), "id".into(), "Uuid".into(), String::new())
        );
    }
```

Confirm the exact `AttrRow` field names by reading `crates/waml-editor/src/inspector.rs:54-58` (`name`, `ty`, `multiplicity`, `visibility`).

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p waml-editor attr_line_parts`
Expected: FAIL — `cannot find function attr_line_parts`.

- [ ] **Step 3: Implement the formatter**

Add near `meta_line` in `inspector_panel.rs`:

```rust
/// Display parts for one attribute row: `(visibility, name, ty, mult)`. Empty
/// visibility and the trivial `"1"` multiplicity are elided. Kept pure so the
/// formatting is unit-tested without a `Cx`; consumed by the attribute row
/// widget (Task 3) and the interim joined line (this task).
fn attr_line_parts(attr: &AttrRow) -> (String, String, String, String) {
    let vis = if attr.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", attr.visibility)
    };
    let mult = if attr.multiplicity.is_empty() || attr.multiplicity == "1" {
        String::new()
    } else {
        format!("  [{}]", attr.multiplicity)
    };
    (vis, attr.name.clone(), attr.ty.clone(), mult)
}
```

Run: `cargo test -p waml-editor attr_line_parts` — Expected: PASS.

- [ ] **Step 4: Create `section_heading.rs`**

```rust
//! `SectionHeading`: one Atlas "eyebrow" label (small SemiBold, `text_dim`) for
//! the inspector body's ATTRIBUTES / RELATIONSHIPS / DESCRIPTION dividers.
//! Pure-view, no interaction: a `#[deref] View` hybrid mirroring `recent_row.rs`
//! with a single `set_text` setter the parent pushes per draw. Reusable for node
//! cards and the node editor.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SectionHeadingBase = #(SectionHeading::register_widget(vm))

    mod.widgets.SectionHeading = set_type_default() do mod.widgets.SectionHeadingBase{
        width: Fill
        height: Fit

        label := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: TextStyle{
                    font_size: 10
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SectionHeading {
    #[deref]
    view: View,
}

impl Widget for SectionHeading {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SectionHeading {
    /// Set the eyebrow text (e.g. "ATTRIBUTES").
    pub fn set_text(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(label)).set_text(cx, s);
    }
}

impl SectionHeadingRef {
    pub fn set_text(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, s);
        }
    }
}
```

- [ ] **Step 5: Declare the module and register in dependency order**

In `main.rs`, add (keep the `mod` list alphabetical — after `mod select_box;`... place near its neighbors):

```rust
mod section_heading;
```

In `app.rs`, between the `select_box` and `inspector_panel` registrations (currently `:1598`/`:1599`), insert:

```rust
        // The inspector body's declared child widgets must register before
        // `inspector_panel`: it mounts `SectionHeading` (and, in later tasks,
        // `AttrRowView` / `RelationshipCardView`) as DSL children, and the DSL
        // resolves `mod.widgets.*` eagerly at `use`-time, not lazily.
        crate::section_heading::script_mod(vm);
```

- [ ] **Step 6: Add the `body` column to the inspector DSL**

In `inspector_panel.rs`, inside `mod.widgets.Inspector { … }`, **after** the `element_bar := View { … }` block and **before** `draw_title +:`, add the `body` column. It is hidden by default (`visible: false`) and revealed only for the `show_picker` path via `set_visible` in `draw_walk`. Interim `attr_lines`/`rel_lines` are plain multi-line `Label`s replaced in Tasks 3/4.

```
        // The diagram/picker body: a declared Turtle column drawn by
        // `self.view.draw_walk`, revealed only when `show_picker` (the
        // classifier-preview path keeps its own immediate-mode body). Rows
        // self-size (Fit) -- no y-offsets, no text measuring.
        body := View {
            width: Fill
            height: Fit
            flow: Down
            visible: false
            padding: Inset{left: 16.0, right: 16.0, top: 0.0, bottom: 16.0}
            spacing: 16.0

            // Full-width hairline under the picker bar (web-header rule).
            divider := View {
                width: Fill
                height: 1.0
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            // Kind line ("Class"): accent, Medium 11.
            kind := Label {
                text: ""
                draw_text +: {
                    color: atlas.accent
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Medium.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
                }
            }
            // Stereotype chips ("<<entity>>"): dim, Regular 11.
            stereo := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
                }
            }

            attr_heading := SectionHeading { }
            // INTERIM (Task 3 replaces with a FlatList<AttrRowView>): the
            // attribute rows joined as one Mono multi-line label.
            attr_lines := Label {
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.4
                    }
                }
            }

            rel_heading := SectionHeading { }
            // INTERIM (Task 4 replaces with a FlatList<RelationshipCardView>):
            // the relationship rows joined as one dim multi-line label.
            rel_lines := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: TextStyle{
                        font_size: 11
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.5
                    }
                }
            }

            desc_heading := SectionHeading { }
            // Editable description body (click-to-edit rect captured in draw_walk).
            desc := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 12
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Medium.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
                }
            }
        }
```

- [ ] **Step 7: Remove the now-orphaned `draw_divider` pen**

The divider is now the `body.divider` View. In the DSL delete the pen line:

```
        // Hairline under the picker bar -- the web header's divider between the
        // subject name row and the body sections.
        draw_divider +: { color: atlas.surface_border }
```

(If working from the committed baseline the comment differs slightly; delete the `draw_divider +: { color: atlas.surface_border }` line whatever its comment.) In `pub struct Inspector`, delete:

```rust
    #[redraw]
    #[live]
    draw_divider: DrawColor,
```

(`draw_divider` is only ever used in the `show_picker` divider draw, which this task replaces — so it must be deleted here or `-D warnings` fails.)

- [ ] **Step 8: Rewrite `draw_walk` — declarative column for `show_picker`, baseline body for `!show_picker`**

Replace the body of `Widget for Inspector::draw_walk` **from the container-draw down**. Keep the collapse/glass preamble. The new shape:

```rust
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let collapsed = self.proj.is_none() || self.folded;
        let mut walk = walk;
        if collapsed {
            walk.height = Size::Fit { min: None, max: None };
        }
        // The diagram/picker body column is a real child; reveal it only when
        // the picker bar is shown and we are not collapsed. The classifier
        // preview (`!show_picker`) keeps its immediate-mode body below.
        let show_body = self.show_picker && !collapsed;
        self.view.widget(cx, ids!(body)).set_visible(cx, show_body);

        // Push the picker-body content BEFORE draw so the column draws it.
        if show_body {
            if let Some(view) = self.proj.clone() {
                self.fill_body_column(cx, &view);
            }
        }

        self.panel.draw(cx, &mut self.view.draw_bg);
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.field_rects.clear();

        if collapsed {
            return DrawStep::done();
        }

        if self.show_picker {
            // Capture the description Label's drawn rect as the click-to-edit
            // target, and paint the edit field-bg over it while editing.
            let desc_rect = self.view.label(cx, ids!(body.desc)).area().rect(cx);
            if self.editing == Some(FieldId::Description) {
                self.draw_field_bg.draw_abs(cx, desc_rect);
            }
            self.field_rects.push((FieldId::Description, desc_rect));
            return DrawStep::done();
        }

        // ---- `show_picker == false`: baseline immediate-mode body (out of
        // scope, unchanged). ----
        let Some(view) = self.proj.clone() else {
            return DrawStep::done();
        };
        // (retain the committed baseline body here verbatim; see Step 9)
        DrawStep::done()
    }
```

- [ ] **Step 9: Preserve the `!show_picker` baseline body verbatim**

Where Step 8's comment marks it, paste the **committed baseline** immediate-mode body (title + kind + stereotypes + attributes + relationships + description) exactly as it exists in the file today, with these adjustments so it compiles under the new shape and only runs for `!show_picker`:

- Delete the `if self.show_picker { self.draw_divider.draw_abs(...) }` divider sub-block (the pen is gone; the divider is the column's job and this branch never drew one for the preview).
- The baseline `if !self.show_picker { … title … }` guard is now always true in this branch — unwrap it to draw the title unconditionally.
- Keep every other line (kind via `draw_kind`/`draw_dim` per baseline, stereotypes, the `ATTRIBUTES`/`RELATIONSHIPS`/`DESCRIPTION` sections via `draw_dim`/`draw_label`/`draw_card`/`draw_name`/`draw_glyph`, the `field_rects.push` for Title and Description) exactly as committed. These pens and the `PAD`/`TITLE_H`/`ROW_H`/`GAP`/`CARD_*`/`GLYPH_W`/`BAR_H` consts stay because this branch still uses them.

This keeps the out-of-scope preview path byte-for-byte behaviourally identical.

- [ ] **Step 10: Implement `fill_body_column`**

Add to `impl Inspector`:

```rust
    /// Push the current projection into the declared `body` column widgets
    /// (kind, stereotypes, the three headings, the interim attribute/relationship
    /// text, and the description). Hides a heading + its rows when that section
    /// is empty. Called each `draw_walk` for the `show_picker` path.
    fn fill_body_column(&mut self, cx: &mut Cx, view: &InspectorView) {
        let kind_line = if view.abstract_flag {
            format!("{}  (abstract)", view.kind_label)
        } else {
            view.kind_label.clone()
        };
        self.view.label(cx, ids!(body.kind)).set_text(cx, &kind_line);

        let stereo = if view.stereotypes.is_empty() {
            String::new()
        } else {
            view.stereotypes
                .iter()
                .map(|s| format!("<<{s}>>"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        self.view.widget(cx, ids!(body.stereo)).set_visible(cx, !stereo.is_empty());
        self.view.label(cx, ids!(body.stereo)).set_text(cx, &stereo);

        // ATTRIBUTES (interim joined Mono lines; Task 3 swaps for a FlatList).
        let has_attrs = !view.attributes.is_empty();
        self.view.widget(cx, ids!(body.attr_heading)).set_visible(cx, has_attrs);
        self.view.widget(cx, ids!(body.attr_lines)).set_visible(cx, has_attrs);
        if has_attrs {
            self.view.widget(cx, ids!(body.attr_heading))
                .as_section_heading()
                .set_text(cx, "ATTRIBUTES");
            let joined = view
                .attributes
                .iter()
                .map(|a| {
                    let (vis, name, ty, mult) = attr_line_parts(a);
                    format!("{vis}{name}: {ty}{mult}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.view.label(cx, ids!(body.attr_lines)).set_text(cx, &joined);
        }

        // RELATIONSHIPS (interim joined lines; Task 4 swaps for a FlatList).
        let has_rels = !view.associations.is_empty();
        self.view.widget(cx, ids!(body.rel_heading)).set_visible(cx, has_rels);
        self.view.widget(cx, ids!(body.rel_lines)).set_visible(cx, has_rels);
        if has_rels {
            self.view.widget(cx, ids!(body.rel_heading))
                .as_section_heading()
                .set_text(cx, "RELATIONSHIPS");
            let joined = view
                .associations
                .iter()
                .map(|r| format!("{} {}  ·  {}", dir_glyph(r.dir), r.other_label, meta_line(r)))
                .collect::<Vec<_>>()
                .join("\n");
            self.view.label(cx, ids!(body.rel_lines)).set_text(cx, &joined);
        }

        // DESCRIPTION (always shown so there is an affordance to add one).
        self.view.widget(cx, ids!(body.desc_heading))
            .as_section_heading()
            .set_text(cx, "DESCRIPTION");
        let desc_text = if self.editing == Some(FieldId::Description) {
            format!("{}\u{2502}", self.edit_buffer)
        } else {
            let t = self.effective_description(view);
            if t.is_empty() { "(click to add)".to_string() } else { t }
        };
        self.view.label(cx, ids!(body.desc)).set_text(cx, &desc_text);
    }
```

- [ ] **Step 11: Run the gate**

Run: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS. Watch specifically for `-D warnings` flagging any leftover unused pen/const — if `draw_divider` (field or DSL) survived, or a const the `!show_picker` branch no longer references, delete it now. Confirm `WidgetRef::as_section_heading()` resolved (proves the registration order is correct and the node is live, not dead).

- [ ] **Step 12: Visual-verify (folded in)**

Launch the worktree native app, click a canvas node in the Orders diagram, capture-by-pid. Confirm the picker body now shows: full-width divider under the bar, accent kind line, stereotype chips (if any), an ATTRIBUTES eyebrow over the attribute lines (Mono), a RELATIONSHIPS eyebrow over the relationship lines, a DESCRIPTION eyebrow over the description (click it — the field-bg + caret appears and typing edits it), roomy 16px section spacing, all self-sizing. Resize the panel / a long name must not clip. Switch to a classifier/source tab (`show_picker == false`) and confirm that preview looks exactly as before this task.

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/section_heading.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): SectionHeading widget + declarative Turtle body column"
```

---

### Task 3: AttrRowView widget + ATTRIBUTES via FlatList

Replace the interim `attr_lines` Label with an `attr_list` `FlatList` whose rows are `AttrRowView` widgets — separate, real-aligned labels (optional visibility, Mono name, literal `": "`, accent type, dim `[mult]`), not a concatenated string. Populate it with the `start_screen.rs` interpose idiom.

**Files:**
- Create: `crates/waml-editor/src/attr_row.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod attr_row;`)
- Modify: `crates/waml-editor/src/app.rs` (register between `section_heading` and `inspector_panel`)
- Modify: `crates/waml-editor/src/inspector_panel.rs` (swap DSL node; add FlatList interpose; drop interim attr Label push)

**Interfaces:**
- Consumes: `attr_line_parts` (Task 2); `SectionHeading` (Task 2).
- Produces: Widget `AttrRowView` with `set_visibility`/`set_name`/`set_ty`/`set_mult` (on both struct and `AttrRowViewRef`); generated `WidgetRef::as_attr_row_view()`.

- [ ] **Step 1: Create `attr_row.rs`**

```rust
//! `AttrRowView`: one inspector attribute row, laid out `flow:Right` with real
//! alignment (NOT a concatenated string): optional visibility, an IBM Plex Mono
//! name, a literal ": ", an accent type, and a dim "[mult]". Pure-view, no
//! interaction -- a `#[deref] View` hybrid mirroring `recent_row.rs`, with
//! granular per-field setters the parent's FlatList loop pushes per row.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.AttrRowViewBase = #(AttrRowView::register_widget(vm))

    mod.widgets.AttrRowView = set_type_default() do mod.widgets.AttrRowViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}

        vis := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        name := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        colon := Label {
            text: ": "
            draw_text +: {
                color: atlas.text
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        ty := Label {
            text: ""
            draw_text +: {
                color: atlas.accent
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
        mult := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: TextStyle{
                    font_size: 11
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AttrRowView {
    #[deref]
    view: View,
}

impl Widget for AttrRowView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AttrRowView {
    pub fn set_visibility(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(vis)).set_text(cx, s);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(name)).set_text(cx, s);
    }
    pub fn set_ty(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(ty)).set_text(cx, s);
    }
    pub fn set_mult(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(mult)).set_text(cx, s);
    }
}

impl AttrRowViewRef {
    pub fn set_visibility(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_visibility(cx, s); }
    }
    pub fn set_name(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_name(cx, s); }
    }
    pub fn set_ty(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_ty(cx, s); }
    }
    pub fn set_mult(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_mult(cx, s); }
    }
}
```

- [ ] **Step 2: Declare the module and register before `inspector_panel`**

In `main.rs` add `mod attr_row;`. In `app.rs`, immediately after `crate::section_heading::script_mod(vm);` (and before `crate::inspector_panel::script_mod(vm);`), add:

```rust
        crate::attr_row::script_mod(vm);
```

- [ ] **Step 3: Swap the interim `attr_lines` Label for a FlatList in the DSL**

In `inspector_panel.rs`, replace the whole interim `attr_lines := Label { … }` block in the `body` column with:

```
            attr_list := FlatList {
                width: Fill
                height: Fit
                flow: Down
                spacing: 6.0

                Row := mod.widgets.AttrRowView { }
            }
```

- [ ] **Step 4: Interpose the attribute list in `draw_walk`, drop the interim push**

In `fill_body_column`, delete the interim ATTRIBUTES block (the `has_attrs` join into `body.attr_lines`) but KEEP the heading visibility toggle:

```rust
        let has_attrs = !view.attributes.is_empty();
        self.view.widget(cx, ids!(body.attr_heading)).set_visible(cx, has_attrs);
        self.view.widget(cx, ids!(body.attr_list)).set_visible(cx, has_attrs);
        if has_attrs {
            self.view.widget(cx, ids!(body.attr_heading))
                .as_section_heading()
                .set_text(cx, "ATTRIBUTES");
        }
```

Then change the container draw loop in `draw_walk` from the plain `while … { }` to an interpose loop that populates `attr_list`. Capture the list uid before the loop (multiple FlatLists are distinguished by uid). Replace:

```rust
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
```

with:

```rust
        let attr_list_uid = self.view.widget(cx, ids!(body.attr_list)).widget_uid();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if !show_body {
                continue;
            }
            if item.widget_uid() == attr_list_uid {
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for attr in &view.attributes {
                            let item_id = LiveId::from_str(&attr.name);
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
        }
```

(The `show_body` binding from Task 2's Step 8 is in scope. `attr_line_parts` is the Task-2 helper. This is the `start_screen.rs:314-345` idiom: stable per-row `item_id`, `list.item` from the `Row` template, per-row `Ref` setters, `row.draw_all`.)

- [ ] **Step 5: Run the gate**

Run: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS. `attr_line_parts` is still consumed (by the interpose), so its tests stay green and it is not orphaned.

- [ ] **Step 6: Visual-verify (folded in)**

Launch, click a canvas node with several attributes (e.g. a Class in the Orders diagram), capture-by-pid. Confirm each attribute renders as `name: Type [mult]` with the name/type in Mono, type in accent, `[mult]` dim, visibility prefix when present — vertically stacked with 6px spacing, self-sizing (a long type must not clip the next row). Confirm the ATTRIBUTES section disappears entirely for a node with no attributes.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/attr_row.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): AttrRowView widget + ATTRIBUTES section via FlatList"
```

---

### Task 4: RelationshipCardView widget + RELATIONSHIPS via FlatList

Replace the interim `rel_lines` Label with a `rel_list` `FlatList` of `RelationshipCardView` widgets — a bordered rounded card (accent-ring `sdf.box` fill, ported from the current `draw_card` idiom) holding a direction-glyph + name row over a dim meta line.

**Files:**
- Create: `crates/waml-editor/src/relationship_card.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod relationship_card;`)
- Modify: `crates/waml-editor/src/app.rs` (register before `inspector_panel`)
- Modify: `crates/waml-editor/src/inspector_panel.rs` (swap DSL node; add FlatList interpose; drop interim rel Label push)

**Interfaces:**
- Consumes: `dir_glyph` + `meta_line` (existing, tested); `SectionHeading` (Task 2).
- Produces: Widget `RelationshipCardView` with `set_glyph`/`set_name`/`set_meta`; generated `WidgetRef::as_relationship_card_view()`.

- [ ] **Step 1: Create `relationship_card.rs`**

```rust
//! `RelationshipCardView`: one inspector relationship card -- a bordered rounded
//! rect (faint field-bg fill ringed by a low-alpha accent stroke, the working
//! box-radius idiom, never `sdf.box(..,0.0)` which floods this fork) holding a
//! Row(accent direction glyph + SemiBold name) over a dim meta line. Pure-view,
//! no interaction -- a `#[deref] View` hybrid mirroring `recent_row.rs`, values
//! pushed per row by the parent's FlatList loop.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.RelationshipCardViewBase = #(RelationshipCardView::register_widget(vm))

    mod.widgets.RelationshipCardView = set_type_default() do mod.widgets.RelationshipCardViewBase{
        width: Fill
        height: Fit
        flow: Down
        padding: Inset{left: 10.0, right: 10.0, top: 10.0, bottom: 10.0}
        spacing: 2.0
        show_bg: true

        // Card material: faint field-bg fill + low-alpha accent ring, rounded
        // corners via the working box-radius idiom.
        draw_bg +: {
            color: atlas.field_bg
            border: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.75, 0.75, self.rect_size.x - 1.5, self.rect_size.y - 1.5, 6.0)
                sdf.fill_keep(vec4(self.color.x, self.color.y, self.color.z, 0.5))
                sdf.stroke(vec4(self.border.x, self.border.y, self.border.z, 0.20), 1.0)
                return sdf.result
            }
        }

        headline := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            spacing: 6.0

            glyph := Label {
                text: ""
                draw_text +: {
                    color: atlas.accent
                    text_style: TextStyle{
                        font_size: 13
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
                }
            }
            name := Label {
                width: Fill
                text: ""
                draw_text +: {
                    color: atlas.text
                    text_style: TextStyle{
                        font_size: 12
                        font_family: FontFamily{
                            latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                        }
                        line_spacing: 1.2
                    }
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
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RelationshipCardView {
    #[deref]
    view: View,
}

impl Widget for RelationshipCardView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RelationshipCardView {
    pub fn set_glyph(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(headline.glyph)).set_text(cx, s);
    }
    pub fn set_name(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(headline.name)).set_text(cx, s);
    }
    pub fn set_meta(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(meta)).set_text(cx, s);
    }
}

impl RelationshipCardViewRef {
    pub fn set_glyph(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_glyph(cx, s); }
    }
    pub fn set_name(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_name(cx, s); }
    }
    pub fn set_meta(&self, cx: &mut Cx, s: &str) {
        if let Some(mut i) = self.borrow_mut() { i.set_meta(cx, s); }
    }
}
```

- [ ] **Step 2: Declare the module and register before `inspector_panel`**

In `main.rs` add `mod relationship_card;`. In `app.rs`, immediately after `crate::attr_row::script_mod(vm);` (and before `crate::inspector_panel::script_mod(vm);`), add:

```rust
        crate::relationship_card::script_mod(vm);
```

- [ ] **Step 3: Swap the interim `rel_lines` Label for a FlatList in the DSL**

In `inspector_panel.rs`, replace the whole interim `rel_lines := Label { … }` block in the `body` column with:

```
            rel_list := FlatList {
                width: Fill
                height: Fit
                flow: Down
                spacing: 8.0

                Row := mod.widgets.RelationshipCardView { }
            }
```

- [ ] **Step 4: Interpose the relationship list in `draw_walk`, drop the interim push**

In `fill_body_column`, replace the interim RELATIONSHIPS block (the `has_rels` join into `body.rel_lines`) with just the heading toggle:

```rust
        let has_rels = !view.associations.is_empty();
        self.view.widget(cx, ids!(body.rel_heading)).set_visible(cx, has_rels);
        self.view.widget(cx, ids!(body.rel_list)).set_visible(cx, has_rels);
        if has_rels {
            self.view.widget(cx, ids!(body.rel_heading))
                .as_section_heading()
                .set_text(cx, "RELATIONSHIPS");
        }
```

Then in the `draw_walk` interpose loop, capture the rel-list uid alongside the attr-list uid and add a second arm. Update the uid capture:

```rust
        let attr_list_uid = self.view.widget(cx, ids!(body.attr_list)).widget_uid();
        let rel_list_uid = self.view.widget(cx, ids!(body.rel_list)).widget_uid();
```

and inside the loop, after the `attr_list_uid` arm, add:

```rust
            if item.widget_uid() == rel_list_uid {
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for (i, assoc) in view.associations.iter().enumerate() {
                            let item_id = LiveId::from_str(&format!("{}-{}-{}", i, assoc.kind, assoc.other_label));
                            let row = list.item(cx, item_id, id!(Row)).unwrap();
                            let rv = row.as_relationship_card_view();
                            rv.set_glyph(cx, dir_glyph(assoc.dir));
                            rv.set_name(cx, &assoc.other_label);
                            rv.set_meta(cx, &meta_line(assoc));
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
```

- [ ] **Step 5: Run the gate**

Run: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS. `dir_glyph` + `meta_line` stay consumed (by the interpose) so their existing tests remain green. `draw_card`/`draw_name`/`draw_glyph` pens are still used by the `!show_picker` baseline body, so they stay (do not delete).

- [ ] **Step 6: Visual-verify (folded in)**

Launch, click a canvas node that has relationships (e.g. `Order` in the Orders diagram), capture-by-pid. Confirm each relationship renders as a bordered rounded card: accent direction glyph (→/←/↔) + SemiBold name on line 1, dim `kind · role · multiplicity` meta on line 2, faint field-bg fill with a low-alpha accent ring, 8px between cards, self-sizing. If any arrow renders as tofu, swap the three `dir_glyph` literals for the ASCII forms `->`/`<-`/`<>` (noted in `dir_glyph`'s doc-comment) and re-verify. Confirm the RELATIONSHIPS section disappears for a node with none, and that the whole inspector still reads cleanly (bold selectbox name, divider, accent kind, aligned Mono attrs, bordered cards, roomy spacing) with no clipping on panel resize.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/relationship_card.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): RelationshipCardView widget + RELATIONSHIPS section via FlatList"
```

---

## Self-Review

**Spec coverage:**
- New widgets `SectionHeading` / `AttrRowView` / `RelationshipCardView` mirroring `recent_row.rs` — Tasks 2 / 3 / 4. ✓
- Type scale ported verbatim into the widgets + column Labels — Global Constraints + Tasks 2–4 DSL. ✓
- Body becomes a declared Turtle `flow:Down` column, populated via the `start_screen.rs` FlatList interpose idiom, with the column order divider → kind → stereotypes → ATTRIBUTES → RELATIONSHIPS → DESCRIPTION — Task 2 (structure) + Tasks 3/4 (variable sections). ✓
- Each variable section uses its own `FlatList` (not a child-vec) with stable per-row `item_id` — Tasks 3/4. ✓
- Description keeps click-to-edit rect + keyboard; only its heading becomes a `SectionHeading` — Task 2 (`fill_body_column` desc push + `desc` rect capture; the existing `handle_event` edit path is unchanged). ✓
- Registration in dependency order (before `inspector_panel`, after `select_box`), dead-node trap addressed — each widget's own task. ✓
- SelectBox flat-header fix: keep bold-14, drop dead `draw_frame` + `AccentFrame`, keep `draw_active` — Task 1. ✓
- `inspector.rs` untouched; `show_picker == false` path left as-is — Global Constraints + Task 2 Steps 8–9. ✓

**Placeholder scan:** No "TBD"/"handle appropriately"/"similar to Task N" — every DSL block and setter is written out; the one retained region (the `!show_picker` baseline body, Task 2 Step 9) is existing code explicitly instructed to be kept verbatim, not a placeholder.

**Type consistency:** `attr_line_parts` returns `(String, String, String, String)` and is consumed identically in Task 2's interim join and Task 3's interpose. `set_text`/`set_visibility`/`set_name`/`set_ty`/`set_mult`/`set_glyph`/`set_meta` names match between each widget's struct impl, its `…Ref` impl, and the inspector call sites. Generated accessors `as_section_heading` / `as_attr_row_view` / `as_relationship_card_view` match the widget type names. FlatList row template id is `Row` (matched by `id!(Row)`) in all three places.

**Risks folded into verification (not separate tasks):**
- Two `FlatList`s stacked in one `Fit` column, each `height:Fit`, distinguished during interpose by `widget_uid`. If Fit-height stacking or uid discrimination misbehaves, it surfaces in the Task 3/4 visual-verify steps.
- `View::set_visible` on the `body` column + section children — proven for the `element_bar` `IconButton`s (`sync_bar_buttons`); the Task 2 visual-verify covers the `show_picker` toggle and the preserved `!show_picker` preview.
