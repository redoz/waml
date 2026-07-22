//! The `Inspector` widget: a right-side panel. Its **container** is a makepad
//! `View` (so it can host real child widgets — the element-picker bar, and, in
//! time, the form of editable field controls the body will grow into). The
//! **body** is still drawn immediate-mode with `DrawText`, exactly like
//! `GraphCanvas` draws node titles, until those controls actually land — the
//! same hybrid `ProjectTree` uses (derefs `View`, yet does manual draws in its
//! `draw_walk`). See `inspector.rs` for the pure `InspectorView` projection.
//!
//! Top bar: a real `SelectBox` child widget (badge + selected label + caret)
//! listing the current diagram's contents (diagram, nodes, source-anchored
//! edges), plus a square pin toggle. Clicking the box opens its `SelectFlyout`
//! card (`App` relays `SelectBox`'s open request to `PopupRoot::show_at`), and
//! a committed pick comes back through the tag-filtered `PopupRoot::closed`
//! queue, resolved via `apply_pick` (which repoints the inspector,
//! inspector-local). Diagram/edge rows are listed but picking them is a no-op
//! for now. The pin is visual-only this cut (its keep-opaque-on-blur purpose
//! is deferred).
//!
//! Step C (inline edit): `Title`/`Description` are click-to-edit. Edits are
//! hand-rolled (no fork `TextInput`) — same convention as `doc_tabs.rs`: rects
//! captured during `draw_walk`, hit-tested on `FingerUp`, keyboard handled via
//! `cx.set_key_focus`/`Hit::KeyDown`/`Hit::TextInput`. Commits go into
//! `overrides` keyed `(subject_key, FieldId)`; the source `Model` is never
//! touched (UX mock only). A changed commit emits `InspectorAction::Edited`,
//! which `App` uses to promote the active preview tab to persisted.

use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::{Icon, IconSet};
use crate::inspector::{
    build_view, effective_field, subject_to_index, ElementKind, ElementRow, FieldId, InspectorView,
    Subject,
};
use crate::node_style::{accent_bucket, AccentBucket};
use crate::panel_glass::PanelGlass;
use crate::popup::base::PopupResult;
use crate::popup::select::{SelectItem, SelectLead};
use crate::select_box::SelectBox;
use crate::tree::kind_of;
use makepad_widgets::*;
use std::collections::HashMap;
use waml::model::{ElementType, Model};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.InspectorBase = #(Inspector::register_widget(vm))

    mod.widgets.Inspector = set_type_default() do mod.widgets.InspectorBase{
        width: Fill
        height: Fill
        show_bg: true
        flow: Down
        // Panel carries the Atlas HUD frame. The container is a `View`, whose
        // `draw_bg` is a `DrawQuad`; the AccentFrame material is inlined onto it
        // (keep in sync with `frame.rs` / `tree_panel.rs`): glass `field_bg`
        // fill ringed by the source-bright accent stroke, 150deg alpha gradient.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            // Glass translucency: `opacity` scales only the interior fill's
            // alpha (frame stroke stays opaque), driven by hover/pin via
            // `PanelGlass` -- see `panel_glass` / `tree_panel`.
            opacity: uniform(1.0)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                let fill = vec4(self.color.x, self.color.y, self.color.z, self.color.w * self.opacity)
                sdf.fill_keep(fill)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }

        // The element-picker bar. Hosts the real `SelectBox` child widget
        // (badge + selected label + caret, its own click handling and open
        // request), then the fold-caret + pin `IconButton`s as laid-out
        // siblings: reserving their width shrinks the `Fill` box so its own
        // caret no longer sits under them (the overlap this replaces). Hidden
        // (`visible: false`) until a diagram feeds the picker -- see
        // `sync_bar_buttons`. The dropped list is the shared `SelectFlyout`
        // surface (routed through `PopupRoot`), so each association row still
        // carries the real `IconSpline` SDF.
        element_bar := View {
            width: Fill
            height: 56.0
            align: Align{x: 0.0, y: 0.5}
            padding: Inset{left: 16.0, right: 16.0}
            spacing: 10.0
            select_box := SelectBox { width: Fill }
            fold_btn := IconButton { visible: false }
            pin_btn := IconButton { visible: false }
        }

        draw_title +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 16
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_label +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_field_bg +: { color: atlas.field_bg }
    }
}

/// Emitted by the inspector. `Edited` is the tab-promotion signal (`App`
/// promotes the active preview tab to persisted on receipt). The
/// element-picker's open-request is now the child `SelectBox`'s own action
/// (`SelectBoxAction::OpenRequested`, forwarded via `take_open_request`), so
/// the inspector no longer emits an open-picker variant here.
#[derive(Clone, Debug, Default)]
pub enum InspectorAction {
    #[default]
    None,
    Edited(String),
}

#[derive(Script, ScriptHook, Widget)]
pub struct Inspector {
    /// The container. Hosts `element_bar`/`element_picker` and carries the HUD
    /// frame bg; the body is drawn manually over it (see `draw_walk`).
    #[deref]
    view: View,

    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,
    #[redraw]
    #[live]
    draw_field_bg: DrawColor,

    /// The flattened read model of the current subject (`None` = empty state).
    #[rust]
    proj: Option<InspectorView>,
    #[rust]
    view_rect: Rect,
    #[rust]
    subject: Subject,
    /// `(subject_key, field) -> edited value`. Never touches `Model`; read
    /// as an override layer on top of `proj` (override-or-model).
    #[rust]
    overrides: HashMap<(String, FieldId), String>,
    /// The field currently being edited, if any. `Some` acquires key focus.
    #[rust]
    editing: Option<FieldId>,
    #[rust]
    edit_buffer: String,
    /// The effective value when editing began — commit is a no-op (no
    /// override write, no `Edited` action) unless the buffer actually changed.
    #[rust]
    edit_original: String,
    #[rust]
    field_rects: Vec<(FieldId, Rect)>,

    /// The current diagram's picker rows (index 0 = placeholder). A picked
    /// visual row maps back to a row here by its stored element index.
    #[rust]
    elements: Vec<ElementRow>,
    /// Whether the element-picker top bar is shown. Diagrams show it (fed via
    /// `set_diagram_elements`); a classifier/package preview hides it
    /// (`set_picker_visible(false)`), floating the body up to the panel top.
    #[rust]
    show_picker: bool,
    /// id -> index into `elements`, rebuilt by `build_select_items` each time
    /// the list is fed. Reverses a committed `SelectItem.id` back to its row.
    #[rust]
    picker_ids: Vec<(LiveId, usize)>,
    /// Glass translucency + hover/pin state machine (shared with the tree
    /// panel; see `panel_glass`). Owns `hovered`/`pinned` and eases the
    /// `draw_bg` `opacity` uniform between translucent-at-rest and opaque.
    #[rust]
    panel: PanelGlass,
    /// Manual body fold. `true` hides the body even when a subject is selected;
    /// `Subject::None` collapses regardless. Toggled by the fold-caret button.
    #[rust]
    folded: bool,
}

// Panel geometry (px). Fixed line advances — no text measuring in this cut.
const PAD: f64 = 16.0;
const TITLE_H: f64 = 26.0;
const ROW_H: f64 = 20.0;
const GAP: f64 = 12.0;
// Bar strip height (matches `element_bar.height` in the DSL). The fold-caret +
// pin affordances in it are now laid-out `IconButton` children, not hand-drawn.
const BAR_H: f64 = 56.0;

/// An association row's display text in the picker popup: just the target end.
/// The model label is `Source -> Target`, but each edge row is drawn beneath its
/// source node (with the `IconSpline` glyph marking it as an association), so the
/// source is redundant. Falls back to the whole label if it isn't `A -> B`.
fn edge_target(label: &str) -> &str {
    label.rsplit(" -> ").next().unwrap_or(label)
}

/// RGB hex (no alpha) -> opaque `Vec4`, matching how the DSL decodes `#xrrggbb`.
fn rgb(hex: u32) -> Vec4 {
    Vec4 {
        x: ((hex >> 16) & 0xff) as f32 / 255.0,
        y: ((hex >> 8) & 0xff) as f32 / 255.0,
        z: (hex & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}

/// Badge fill for an accent bucket (Atlas `bucket_*` swatches; `None` and
/// `Unknown` share the neutral slate).
fn bucket_color(b: AccentBucket) -> Vec4 {
    match b {
        AccentBucket::Interface => rgb(0x1496dc),
        AccentBucket::Enum => rgb(0x00b4d2),
        AccentBucket::Note => rgb(0x14bea0),
        AccentBucket::Actor => rgb(0x5a6ef0),
        AccentBucket::UseCase => rgb(0xe69614),
        AccentBucket::Package => rgb(0x3cbe5a),
        AccentBucket::Behavior => rgb(0xeb4678),
        AccentBucket::None | AccentBucket::Unknown => rgb(0x64748b),
    }
}

/// Leading visual for a node picker row: the shared catalog glyph for the
/// element's type when one exists, else a coloured monogram badge. Every
/// modelled UML kind resolves to an icon (`Customer`, a `Class`, leads with
/// `PanelTop` -- the same glyph the tree and doc-tab strip already draw for
/// it); only `Unknown` types, which have no HUD glyph, fall back to the badge.
fn node_lead(ty: &ElementType, letter: String) -> SelectLead {
    match IconSet::icon_for(kind_of(ty)) {
        Some(icon) => SelectLead::Icon(icon),
        None => SelectLead::Badge {
            color: bucket_color(accent_bucket(ty)),
            letter,
        },
    }
}

impl Widget for Inspector {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Drive the container first.
        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        // All hit rects (pin, caret, pencil, inline-edit fields) are recorded
        // in `draw_walk` off `self.view.area().rect(cx)`, which *during a
        // draw* reports the pre-alignment turtle origin (x≈0). This panel
        // lives in a right-aligned parent, so the finished draw list is
        // shifted right by the panel's x — the glyphs render there, but the
        // stored rects keep the unshifted origin. Pointer events arrive in that
        // shifted (post-alignment) space, so translate the event point back
        // into draw-time space by the offset between the two before any
        // `contains` test. (The picker field itself is now the child
        // `SelectBox`'s own hit rect, event-time-anchored — see `select_box.rs`.)
        // `area().rect` at event time is the real (post-alignment) on-screen
        // rect; `self.view_rect` is the pre-alignment draw-time rect (x≈0 in
        // this right-aligned parent). `hit_off` is the shift between them.
        let panel_rect = self.view.area().rect(cx);
        let hit_off = panel_rect.pos - self.view_rect.pos;

        // Glass hover + opacity easing off the real on-screen rect (geometric
        // MouseMove containment, not `Hit::FingerHover*` -- the child
        // `SelectBox` / picker claim the pointer's hover first, so a hit-based
        // test leaves the panel stuck translucent under the cursor; see
        // `panel_glass`).
        if self.panel.handle_event(cx, event, panel_rect) {
            self.view.redraw(cx);
        }

        // Fold-caret + pin `IconButton` children emit their clicks as widget
        // actions; read them here. The pin toggles the panel's keep-opaque lock;
        // the caret folds/unfolds the body (only meaningful with a subject --
        // with none the panel is already collapsed).
        if let Event::Actions(actions) = event {
            if self
                .view
                .widget(cx, ids!(element_bar.pin_btn))
                .as_icon_button()
                .clicked(actions)
            {
                self.panel.toggle_pin(cx);
                self.sync_bar_buttons(cx);
                self.view.redraw(cx);
            }
            if self
                .view
                .widget(cx, ids!(element_bar.fold_btn))
                .as_icon_button()
                .clicked(actions)
                && self.proj.is_some()
            {
                self.folded = !self.folded;
                self.sync_bar_buttons(cx);
                self.view.redraw(cx);
            }
        }

        match event.hits_with_capture_overload(cx, self.view.area(), false) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let p = fe.abs - hit_off;
                if self.editing.is_some() {
                    self.commit_edit(cx, uid);
                }
                for (field, rect) in self.field_rects.clone() {
                    if rect.contains(p) {
                        self.begin_edit(cx, field);
                        break;
                    }
                }
            }
            Hit::KeyFocusLost(_) => {
                self.commit_edit(cx, uid);
            }
            Hit::KeyDown(ke) if self.editing.is_some() => match ke.key_code {
                KeyCode::ReturnKey => self.commit_edit(cx, uid),
                KeyCode::Escape => self.cancel_edit(cx),
                KeyCode::Backspace => {
                    self.edit_buffer.pop();
                    self.view.redraw(cx);
                }
                _ => {}
            },
            Hit::TextInput(ti) if self.editing.is_some() => {
                for ch in ti.input.chars() {
                    if !ch.is_control() {
                        self.edit_buffer.push(ch);
                    }
                }
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Draw the container (HUD frame bg) and the bar child (dropdown).
        // Collapsed = nothing selected, or the user folded the body via the
        // caret. Collapse the frame to hug just the bar; the parent wrapper
        // aligns this panel top-right, so a `Fit` height floats it to the top.
        let collapsed = self.proj.is_none() || self.folded;
        let mut walk = walk;
        if collapsed {
            walk.height = Size::Fit {
                min: None,
                max: None,
            };
        }
        // Glass translucency: seed + push the eased `opacity` uniform before the
        // container draws its frame bg. Opaque when hovered/pinned, else
        // translucent so the canvas shows through. Replaces the old dimming
        // scrim (which painted last, over everything). See `panel_glass`.
        self.panel.draw(cx, &mut self.view.draw_bg);
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.field_rects.clear();

        // The fold-caret + pin affordances are laid-out `IconButton` children of
        // `element_bar` (drawn above by `self.view.draw_walk`); their glyph +
        // visibility track `folded`/`pinned`/`show_picker` via `sync_bar_buttons`
        // on each state change, so there's nothing to hand-draw here.

        // Body, below the bar (or floated to the top when the bar is hidden).
        // When collapsed the frame already hugs the bar -- the placeholder lives
        // in the field itself, so there's no body.
        if collapsed {
            return DrawStep::done();
        }
        let Some(view) = self.proj.clone() else {
            return DrawStep::done();
        };
        let field_w = rect.size.x - PAD * 2.0;

        let bar_h = if self.show_picker { BAR_H } else { 0.0 };
        let x = rect.pos.x + PAD;
        let mut y = rect.pos.y + bar_h + PAD;

        // Title: click-to-edit.
        let title_rect = Rect {
            pos: dvec2(x, y),
            size: dvec2(field_w, TITLE_H),
        };
        if self.editing == Some(FieldId::Title) {
            self.draw_field_bg.draw_abs(cx, title_rect);
            self.draw_title
                .draw_abs(cx, dvec2(x, y), &format!("{}\u{2502}", self.edit_buffer));
        } else {
            self.draw_title
                .draw_abs(cx, dvec2(x, y), &self.effective_title(&view));
        }
        self.field_rects.push((FieldId::Title, title_rect));
        y += TITLE_H;

        // Kind + abstract badge, e.g. "Class  (abstract)". Read-only breadth (U6).
        let kind_line = if view.abstract_flag {
            format!("{}  (abstract)", view.kind_label)
        } else {
            view.kind_label.clone()
        };
        self.draw_dim.draw_abs(cx, dvec2(x, y), &kind_line);
        y += ROW_H;

        // Stereotype chips, e.g. "<<aggregateRoot>> <<entity>>". Read-only breadth (U6).
        if !view.stereotypes.is_empty() {
            let chips = view
                .stereotypes
                .iter()
                .map(|s| format!("<<{s}>>"))
                .collect::<Vec<_>>()
                .join(" ");
            self.draw_dim.draw_abs(cx, dvec2(x, y), &chips);
            y += ROW_H;
        }
        y += GAP;

        if !view.attributes.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "ATTRIBUTES");
            y += ROW_H;
            for attr in &view.attributes {
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
                let line = format!("{vis}{}: {}{mult}", attr.name, attr.ty);
                self.draw_label.draw_abs(cx, dvec2(x, y), &line);
                y += ROW_H;
            }
            y += GAP;
        }

        // Associations: read-only, derived from Model::edges (U6 breadth). Not
        // click-to-edit -- there's no single scalar override target for a
        // relationship yet.
        if !view.associations.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "ASSOCIATIONS");
            y += ROW_H;
            for assoc in &view.associations {
                let line = format!("{} {} {}", assoc.direction, assoc.other_label, assoc.kind);
                self.draw_label.draw_abs(cx, dvec2(x, y), &line);
                y += ROW_H;
            }
            y += GAP;
        }

        // Description: click-to-edit. Renders even when the model has none,
        // so there's always an affordance to add one.
        self.draw_dim.draw_abs(cx, dvec2(x, y), "DESCRIPTION");
        y += ROW_H;
        let desc_rect = Rect {
            pos: dvec2(x, y),
            size: dvec2(field_w, ROW_H),
        };
        if self.editing == Some(FieldId::Description) {
            self.draw_field_bg.draw_abs(cx, desc_rect);
            self.draw_label
                .draw_abs(cx, dvec2(x, y), &format!("{}\u{2502}", self.edit_buffer));
        } else {
            let text = self.effective_description(&view);
            if text.is_empty() {
                self.draw_dim.draw_abs(cx, dvec2(x, y), "(click to add)");
            } else {
                self.draw_label.draw_abs(cx, dvec2(x, y), &text);
            }
        }
        self.field_rects.push((FieldId::Description, desc_rect));

        DrawStep::done()
    }
}

impl Inspector {
    /// Point the inspector at `subject`, rebuilding the projection and syncing
    /// the picker's selected row. Overrides persist across subject switches
    /// (keyed per subject); an in-progress edit is discarded uncommitted.
    pub fn set_subject(&mut self, cx: &mut Cx, model: &Model, subject: Subject) {
        self.proj = build_view(model, &subject);
        self.subject = subject;
        self.editing = None;
        // Switching subject clears a manual fold; the new element shows expanded.
        self.folded = false;
        // Re-mark the box's selection so a pick made elsewhere (canvas/tree)
        // shows up in the picker bar too.
        let sel = subject_to_index(&self.elements, &self.subject);
        let sel_in_items = if sel == 0 { None } else { Some(sel - 1) };
        if let Some(mut b) = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
        {
            b.set_selected(cx, sel_in_items);
        }
        self.sync_bar_buttons(cx);
        self.view.redraw(cx);
    }

    /// Sync the fold-caret + pin `IconButton` children to the current state:
    /// glyph (collapse/expand, pin/pin-off), the pin's lit state (`pinned`), and
    /// visibility (only shown while the picker bar is). Called on every state
    /// change that affects them; the buttons draw themselves as `element_bar`
    /// children.
    fn sync_bar_buttons(&mut self, cx: &mut Cx) {
        let collapsed = self.proj.is_none() || self.folded;
        let pinned = self.panel.pinned;
        let vis = self.show_picker;

        let fold = self.view.widget(cx, ids!(element_bar.fold_btn));
        fold.set_visible(cx, vis);
        fold.as_icon_button().set_icon(
            cx,
            if collapsed {
                Icon::ListExpand
            } else {
                Icon::ListCollapse
            },
        );

        let pin = self.view.widget(cx, ids!(element_bar.pin_btn));
        pin.set_visible(cx, vis);
        pin.as_icon_button()
            .set_icon(cx, if pinned { Icon::Pin } else { Icon::PinOff });
        pin.as_icon_button().set_active(cx, pinned);
    }

    /// Build the picker rows as `SelectItem`s and record their id→index map (for
    /// `apply_pick`). Node rows lead with their catalog glyph (see `node_lead`,
    /// falling back to a per-type badge for `Unknown` types) and are enabled;
    /// edge rows lead with the spline glyph (target-end label) and are disabled;
    /// the root diagram row leads with the `Frame` glyph and is disabled. Index
    /// 0 (placeholder) is skipped.
    fn build_select_items(&mut self, model: &Model) -> Vec<SelectItem> {
        self.picker_ids.clear();
        let mut items = Vec::new();
        for idx in 1..self.elements.len() {
            let row = self.elements[idx].clone();
            let id = LiveId::from_str(&row.key);
            self.picker_ids.push((id, idx));
            let selected = subject_to_index(&self.elements, &self.subject) == idx;
            let (lead, label, enabled) = match row.kind {
                ElementKind::Node => {
                    let lead = model
                        .nodes
                        .iter()
                        .find(|n| n.key == row.key)
                        .map(|n| {
                            let letter = build_view(model, &Subject::Classifier(row.key.clone()))
                                .and_then(|v| v.kind_label.chars().next())
                                .map(|c| c.to_uppercase().to_string())
                                .unwrap_or_default();
                            node_lead(&n.ty, letter)
                        })
                        .unwrap_or(SelectLead::Badge {
                            color: bucket_color(AccentBucket::None),
                            letter: String::new(),
                        });
                    (lead, row.label.clone(), true)
                }
                ElementKind::Edge => (
                    SelectLead::Icon(Icon::Spline),
                    edge_target(&row.label).to_string(),
                    false,
                ),
                // The root diagram row leads with the `Frame` glyph -- distinct
                // from any node's catalog icon, marking it as the container.
                ElementKind::Diagram => {
                    (SelectLead::Icon(Icon::Frame), row.label.clone(), false)
                }
                _ => (SelectLead::None, row.label.clone(), false),
            };
            items.push(SelectItem {
                id,
                lead,
                label,
                selected,
                enabled,
            });
        }
        items
    }

    /// Feed the element-picker the current diagram's rows. Called by `App`
    /// whenever the current diagram changes. `model` is needed to look up each
    /// node's `AccentBucket` for the box/flyout badges.
    pub fn set_diagram_elements(&mut self, cx: &mut Cx, model: &Model, rows: Vec<ElementRow>) {
        self.elements = rows;
        // Feeding diagram rows implies a diagram tab: show the picker bar.
        self.show_picker = true;
        let items = self.build_select_items(model);
        let sel = subject_to_index(&self.elements, &self.subject);
        let sel_in_items = if sel == 0 { None } else { Some(sel - 1) };
        if let Some(mut b) = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
        {
            b.set_items(cx, items);
            b.set_selected(cx, sel_in_items);
        }
        self.sync_bar_buttons(cx);
        self.view.redraw(cx);
    }

    /// Show/hide the element-picker top bar. Hidden while previewing a
    /// classifier/package (no diagram to pick elements from); the body then
    /// floats up to the panel top.
    pub fn set_picker_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.show_picker = visible;
        self.sync_bar_buttons(cx);
        self.view.redraw(cx);
    }

    /// Resolve a committed `PopupItem.id` (from `PopupRoot::closed`) back to its
    /// element and repoint the inspector. Returns the new subject, or `None` if
    /// the id wasn't a pickable (node) element in the current list.
    pub fn apply_pick(&mut self, cx: &mut Cx, model: &Model, id: LiveId) -> Option<Subject> {
        let idx = self
            .picker_ids
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, x)| *x)?;
        let row = self.elements.get(idx)?;
        if !matches!(row.kind, ElementKind::Node) {
            return None;
        }
        let subject = Subject::Classifier(row.key.clone());
        self.set_subject(cx, model, subject.clone());
        Some(subject)
    }

    fn subject_key(&self) -> Option<String> {
        match &self.subject {
            Subject::Classifier(key) => Some(key.clone()),
            Subject::None => None,
        }
    }

    fn effective_title(&self, view: &InspectorView) -> String {
        let key = self.subject_key();
        let over = key
            .as_ref()
            .and_then(|k| self.overrides.get(&(k.clone(), FieldId::Title)));
        effective_field(view, FieldId::Title, over)
    }

    fn effective_description(&self, view: &InspectorView) -> String {
        let key = self.subject_key();
        let over = key
            .as_ref()
            .and_then(|k| self.overrides.get(&(k.clone(), FieldId::Description)));
        effective_field(view, FieldId::Description, over)
    }

    fn effective_value(&self, field: FieldId) -> String {
        let Some(view) = &self.proj else {
            return String::new();
        };
        match field {
            FieldId::Title => self.effective_title(view),
            FieldId::Description => self.effective_description(view),
        }
    }

    fn begin_edit(&mut self, cx: &mut Cx, field: FieldId) {
        if self.subject_key().is_none() {
            return; // Empty state: nothing to attach an override to.
        }
        let current = self.effective_value(field);
        self.editing = Some(field);
        self.edit_buffer = current.clone();
        self.edit_original = current;
        cx.set_key_focus(self.view.area());
        self.view.redraw(cx);
    }

    fn commit_edit(&mut self, cx: &mut Cx, uid: WidgetUid) {
        let Some(field) = self.editing.take() else {
            return;
        };
        if let Some(key) = self.subject_key() {
            if self.edit_buffer != self.edit_original {
                self.overrides
                    .insert((key.clone(), field), self.edit_buffer.clone());
                cx.widget_action(uid, InspectorAction::Edited(key));
            }
        }
        self.view.redraw(cx);
    }

    fn cancel_edit(&mut self, cx: &mut Cx) {
        self.editing = None;
        self.view.redraw(cx);
    }

    /// Convenience reader for `App`, mirroring `DocTabs::tab_action`.
    pub fn edited(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            InspectorAction::Edited(key) => Some(key),
            _ => None,
        }
    }

    /// Forward the child `SelectBox`'s open request (App relays it to
    /// `PopupRoot`). `None` unless the box asked to open this pass.
    pub fn take_open_request(
        &self,
        cx: &mut Cx,
        actions: &Actions,
    ) -> Option<(Rect, f64, Vec<SelectItem>)> {
        self.view
            .widget(cx, ids!(element_bar.select_box))
            .borrow::<SelectBox>()?
            .open_request(actions)
    }

    /// The flyout closed. Clear the box's active state; on a committed node pick
    /// repoint the inspector via `apply_pick`.
    pub fn on_picker_closed(&mut self, cx: &mut Cx, model: &Model, result: PopupResult) {
        let picked = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
            .and_then(|mut b| b.on_closed(cx, result));
        if let Some(id) = picked {
            self.apply_pick(cx, model, id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_target_returns_target_end() {
        // Association rows show only the target; the source is implied by the
        // node the row sits under (plus the spline glyph marking it an edge).
        assert_eq!(edge_target("Order -> Customer"), "Customer");
    }

    #[test]
    fn edge_target_falls_back_to_whole_label() {
        assert_eq!(edge_target("Standalone"), "Standalone");
    }

    // A `Customer` node is a UML `Class`; the picker must lead it with the
    // shared catalog glyph (the same one the tree draws), never the grey
    // monogram badge that regressed it to a "C in a box".
    #[test]
    fn node_lead_uses_catalog_icon_for_known_type() {
        let lead = node_lead(
            &ElementType::Uml(waml::model::UmlMetaclass::Class),
            "C".into(),
        );
        assert!(
            matches!(lead, SelectLead::Icon(Icon::PanelTop)),
            "Class node should lead with its catalog icon, got {lead:?}"
        );
    }

    // Unknown types have no HUD glyph, so the monogram badge is the correct
    // fallback -- the letter survives for them.
    #[test]
    fn node_lead_falls_back_to_badge_for_unknown_type() {
        let lead = node_lead(&ElementType::Unknown("Widget".into()), "W".into());
        assert!(
            matches!(lead, SelectLead::Badge { ref letter, .. } if letter == "W"),
            "Unknown node should fall back to the monogram badge, got {lead:?}"
        );
    }
}
