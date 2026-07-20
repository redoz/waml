//! The `Inspector` widget: a right-side panel. Its **container** is a makepad
//! `View` (so it can host real child widgets — the element-picker bar, and, in
//! time, the form of editable field controls the body will grow into). The
//! **body** is still drawn immediate-mode with `DrawText`, exactly like
//! `GraphCanvas` draws node titles, until those controls actually land — the
//! same hybrid `ProjectTree` uses (derefs `View`, yet does manual draws in its
//! `draw_walk`). See `inspector.rs` for the pure `InspectorView` projection.
//!
//! Top bar: an element-picker `DropDown` listing the current diagram's contents
//! (diagram, nodes, source-anchored edges), plus a square pin toggle. Picking a
//! node row repoints the inspector (inspector-local; emits
//! `InspectorAction::ElementPicked`, which `App` resolves via `set_subject`).
//! Diagram/edge rows are listed but selecting them is a no-op for now. The pin
//! is visual-only this cut (its keep-opaque-on-blur purpose is deferred).
//!
//! Step C (inline edit): `Title`/`Description` are click-to-edit. Edits are
//! hand-rolled (no fork `TextInput`) — same convention as `doc_tabs.rs`: rects
//! captured during `draw_walk`, hit-tested on `FingerUp`, keyboard handled via
//! `cx.set_key_focus`/`Hit::KeyDown`/`Hit::TextInput`. Commits go into
//! `overrides` keyed `(subject_key, FieldId)`; the source `Model` is never
//! touched (UX mock only). A changed commit emits `InspectorAction::Edited`,
//! which `App` uses to promote the active preview tab to persisted.

use crate::icons::IconSet;
use crate::inspector::{
    build_view, effective_field, subject_to_index, ElementKind, ElementRow, FieldId,
    InspectorView, Subject, PICKER_PLACEHOLDER,
};
use crate::node_style::{accent_bucket, AccentBucket};
use makepad_widgets::*;
use std::collections::HashMap;
use waml::model::Model;

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
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }

        // The element-picker bar. `element_bar` is an empty spacer that just
        // reserves the panel's top strip (so the container's `Fit` height keeps
        // the bar when collapsed); everything in it -- the picker field (badge +
        // selected label), the pencil/caret/pin glyphs, and the owned popup list
        // -- is hand-drawn immediate-mode in `draw_walk`. The picker is no longer
        // a fork `DropDown`: its popup is drawn here so each association row can
        // carry the real `IconSpline` SDF (the fork `PopupMenu` is label-only).
        element_bar := View {
            width: Fill
            height: 56.0
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
        // Owned element-picker popup (replaces the fork DropDown so each edge row
        // can draw the real `IconSpline` SDF). `draw_popup_bg` is the frosted
        // Atlas HUD sheet; `draw_row_hi` is the hover/selected row wash;
        // `draw_row_text`/`draw_row_sel` are the row label (dark / accent-when-
        // selected); `draw_icon_edge` is a colour-only holder whose `color` is
        // copied onto `icons.spline` per draw (no RGBA crosses Rust).
        draw_popup_bg: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_row_hi +: { color: atlas.selection }
        draw_row_text +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 13
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
            }
        }
        draw_row_sel +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 13
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
            }
        }
        draw_icon_edge +: { color: atlas.accent }
        // Pin box: a source-bright frame around a fill that reads accent when
        // pinned, empty (field_bg) when not.
        draw_pin_frame +: { color: atlas.frame_hi }
        draw_pin_on +: { color: atlas.accent }
        draw_pin_off +: { color: atlas.field_bg }
        // Type-badge: solid per-kind square (colour set at draw time) with the
        // kind initial (white) drawn on top.
        draw_badge +: { color: atlas.bucket_slate }
        draw_badge_text +: {
            color: #xffffff
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
            }
        }
        // Pencil (edit affordance) + collapse/unfold caret. Both grey glyphs.
        draw_pencil +: {
            color: uniform(atlas.text_dim)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let s = self.rect_size
                // Pencil body: one diagonal stroke, with a short tip mark.
                sdf.move_to(s.x * 0.30, s.y * 0.70)
                sdf.line_to(s.x * 0.70, s.y * 0.30)
                sdf.stroke(self.color, 2.0)
                sdf.move_to(s.x * 0.22, s.y * 0.78)
                sdf.line_to(s.x * 0.32, s.y * 0.68)
                sdf.stroke(self.color, 2.0)
                return sdf.result
            }
        }
        draw_caret_down +: {
            color: uniform(atlas.text_dim)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let mx = self.rect_size.x * 0.5
                let my = self.rect_size.y * 0.5
                let w = 5.0
                let h = 3.0
                sdf.move_to(mx - w, my - h)
                sdf.line_to(mx, my + h)
                sdf.line_to(mx + w, my - h)
                sdf.stroke(self.color, 1.5)
                return sdf.result
            }
        }
        draw_caret_up +: {
            color: uniform(atlas.text_dim)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let mx = self.rect_size.x * 0.5
                let my = self.rect_size.y * 0.5
                let w = 5.0
                let h = 3.0
                sdf.move_to(mx - w, my + h)
                sdf.line_to(mx, my - h)
                sdf.line_to(mx + w, my + h)
                sdf.stroke(self.color, 1.5)
                return sdf.result
            }
        }
    }
}

/// Emitted by the inspector. `Edited` is the tab-promotion signal (`App`
/// promotes the active preview tab to persisted on receipt). `ElementPicked` is
/// a node pick from the element-picker dropdown (`App` resolves it via
/// `set_subject`, the same path a canvas/tab selection takes).
#[derive(Clone, Debug, Default)]
pub enum InspectorAction {
    #[default]
    None,
    Edited(String),
    ElementPicked(String),
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

    /// Owned element-picker popup. `draw_popup_bg` is the HUD sheet (AccentFrame);
    /// `draw_row_hi` washes the hovered/selected row; `draw_row_text`/`draw_row_sel`
    /// are the row label (dark / accent when selected). `icons` supplies the real
    /// `IconSpline` SDF drawn on association rows, tinted from `draw_icon_edge`.
    #[live]
    draw_popup_bg: DrawColor,
    #[live]
    draw_row_hi: DrawColor,
    #[live]
    draw_row_text: DrawText,
    #[live]
    draw_row_sel: DrawText,
    #[live]
    draw_icon_edge: DrawColor,
    #[live]
    icons: IconSet,
    /// Pin box: frame + one of two fills (on/off), picked by `pinned`.
    #[live]
    draw_pin_frame: DrawColor,
    #[live]
    draw_pin_on: DrawColor,
    #[live]
    draw_pin_off: DrawColor,
    /// Left type-badge: a per-kind coloured square (`draw_badge.color` is set at
    /// draw time from the subject's `AccentBucket`) with the kind initial on top.
    #[live]
    draw_badge: DrawColor,
    #[live]
    draw_badge_text: DrawText,
    /// Pencil (edit affordance, visual-only this cut) and the collapse/unfold
    /// caret. Hand-drawn glyphs in the bar's right gap. The caret has two
    /// shaders (up = fold when body shown, down = unfold when collapsed) since
    /// this fork can't flip a `DrawQuad` instance at runtime.
    #[live]
    draw_pencil: DrawQuad,
    #[live]
    draw_caret_up: DrawQuad,
    #[live]
    draw_caret_down: DrawQuad,

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
    /// Owned-popup state: whether the list is open, the hit rects of the visible
    /// rows (each paired with its index into `elements`), and the hovered row.
    #[rust]
    picker_open: bool,
    #[rust]
    picker_field_rect: Rect,
    #[rust]
    picker_rects: Vec<(usize, Rect)>,
    #[rust]
    picker_hover: Option<usize>,
    /// Pin toggle. Visual-only this cut (keep-opaque-on-blur is deferred).
    #[rust]
    pinned: bool,
    #[rust]
    pin_rect: Rect,
    /// Manual body fold. `true` hides the body even when a subject is selected;
    /// `Subject::None` collapses regardless. Toggled by the caret.
    #[rust]
    folded: bool,
    /// Badge fill colour + kind initial for the current subject, computed in
    /// `set_subject` from the node's `AccentBucket`.
    #[rust]
    badge_color: Vec4,
    #[rust]
    badge_letter: String,
    #[rust]
    pencil_rect: Rect,
    #[rust]
    caret_rect: Rect,
}

// Panel geometry (px). Fixed line advances — no text measuring in this cut.
const PAD: f64 = 16.0;
const TITLE_H: f64 = 26.0;
const ROW_H: f64 = 20.0;
const GAP: f64 = 12.0;
// Bar strip height (matches `element_bar.height` in the DSL) and the icon glyphs
// drawn in its reserved right gap (pencil, caret, pin -- right to left).
const BAR_H: f64 = 56.0;
const PIN_SIZE: f64 = 24.0;
const PIN_MARGIN: f64 = 12.0;
const ICON: f64 = 24.0;
const ICON_GAP: f64 = 8.0;
// Left type-badge (drawn over the field's left inset).
const BADGE_SIZE: f64 = 24.0;
// Owned element-picker popup geometry (px).
const POPUP_PAD: f64 = 8.0; // sheet inset from the panel edges + inner padding
const ROW_H_PICK: f64 = 30.0; // per-row height in the popup list
const ROW_ICON: f64 = 16.0; // spline-icon box on association rows

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

impl Widget for Inspector {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Drive the container first.
        self.view.handle_event(cx, event, scope);

        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.view.area(), true) {
            // Track the hovered popup row so it can wash under the pointer.
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) if self.picker_open => {
                let hov = self
                    .picker_rects
                    .iter()
                    .find(|(_, r)| r.contains(fe.abs))
                    .map(|(i, _)| *i);
                if hov != self.picker_hover {
                    self.picker_hover = hov;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                // Owned picker popup: a row picks, anything else closes.
                if self.picker_open {
                    if let Some(idx) = self
                        .picker_rects
                        .iter()
                        .find(|(_, r)| r.contains(fe.abs))
                        .map(|(i, _)| *i)
                    {
                        self.choose_element(cx, idx);
                    } else {
                        self.close_picker(cx);
                    }
                    return;
                }
                // Closed: the picker field opens the list.
                if self.picker_field_rect.contains(fe.abs) {
                    self.open_picker(cx);
                    return;
                }
                if self.pin_rect.contains(fe.abs) {
                    self.pinned = !self.pinned;
                    self.view.redraw(cx);
                    return;
                }
                // Caret folds/unfolds the body (only meaningful when a subject
                // is set; with none the panel is already collapsed).
                if self.caret_rect.contains(fe.abs) {
                    if self.proj.is_some() {
                        self.folded = !self.folded;
                        self.view.redraw(cx);
                    }
                    return;
                }
                if self.editing.is_some() {
                    self.commit_edit(cx, uid);
                }
                for (field, rect) in self.field_rects.clone() {
                    if rect.contains(fe.abs) {
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
        // Visible popup rows = every element except the index-0 placeholder.
        let n_rows = self.elements.len().saturating_sub(1);
        let sheet_h = POPUP_PAD * 2.0 + (n_rows as f64) * ROW_H_PICK;
        let mut walk = walk;
        if self.picker_open {
            // Grow the frame to contain the bar + the dropped list, so the list
            // isn't clipped by the panel (the body is hidden while it's open).
            walk.height = Size::Fixed(BAR_H + sheet_h);
        } else if collapsed {
            walk.height = Size::Fit {
                min: None,
                max: None,
            };
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.field_rects.clear();
        self.picker_rects.clear();

        let cy = rect.pos.y + BAR_H * 0.5;

        // Left type-badge, over the field's left inset (only when a subject is
        // set). `draw_badge.color` was computed per-kind in `set_subject`.
        if self.proj.is_some() {
            let badge = Rect {
                pos: dvec2(rect.pos.x + PAD + 4.0, cy - BADGE_SIZE * 0.5),
                size: dvec2(BADGE_SIZE, BADGE_SIZE),
            };
            self.draw_badge.color = self.badge_color;
            self.draw_badge.draw_abs(cx, badge);
            if !self.badge_letter.is_empty() {
                self.draw_badge_text
                    .draw_abs(cx, dvec2(badge.pos.x + 7.0, badge.pos.y + 4.0), &self.badge_letter);
            }
        }

        // Right glyph cluster, right -> left: pin, caret, pencil.
        let pin = Rect {
            pos: dvec2(
                rect.pos.x + rect.size.x - PIN_MARGIN - PIN_SIZE,
                cy - PIN_SIZE * 0.5,
            ),
            size: dvec2(PIN_SIZE, PIN_SIZE),
        };
        self.pin_rect = pin;
        self.draw_pin_frame.draw_abs(cx, pin);
        let inset = 1.5;
        let inner = Rect {
            pos: dvec2(pin.pos.x + inset, pin.pos.y + inset),
            size: dvec2(pin.size.x - inset * 2.0, pin.size.y - inset * 2.0),
        };
        if self.pinned {
            self.draw_pin_on.draw_abs(cx, inner);
        } else {
            self.draw_pin_off.draw_abs(cx, inner);
        }

        // Collapse/unfold caret: up (fold) when the body is showing, down
        // (unfold) when collapsed.
        let caret = Rect {
            pos: dvec2(pin.pos.x - ICON_GAP - ICON, cy - ICON * 0.5),
            size: dvec2(ICON, ICON),
        };
        self.caret_rect = caret;
        if collapsed {
            self.draw_caret_down.draw_abs(cx, caret);
        } else {
            self.draw_caret_up.draw_abs(cx, caret);
        }

        // Pencil (edit affordance, visual-only this cut).
        let pencil = Rect {
            pos: dvec2(caret.pos.x - ICON_GAP - ICON, cy - ICON * 0.5),
            size: dvec2(ICON, ICON),
        };
        self.pencil_rect = pencil;
        self.draw_pencil.draw_abs(cx, pencil);

        // Picker field label: the selected element's title (subdued, text_dim),
        // or the placeholder when nothing is picked. Sits right of the badge; the
        // whole left strip of the bar is the click target that opens the list.
        let sel = subject_to_index(&self.elements, &self.subject);
        let field_label = self
            .elements
            .get(sel)
            .map(|r| r.label.clone())
            .unwrap_or_else(|| PICKER_PLACEHOLDER.to_string());
        let label_x = if self.proj.is_some() {
            rect.pos.x + PAD + 4.0 + BADGE_SIZE + 10.0
        } else {
            rect.pos.x + PAD + 4.0
        };
        self.draw_dim
            .draw_abs(cx, dvec2(label_x, cy - 7.0), &field_label);
        self.picker_field_rect = Rect {
            pos: rect.pos,
            size: dvec2((pencil.pos.x - ICON_GAP - rect.pos.x).max(0.0), BAR_H),
        };

        // Owned popup list, dropped below the bar. It replaces the body while
        // open (the frame was sized to bar + list above), so return after.
        if self.picker_open {
            self.draw_picker_list(cx, rect, n_rows, sel);
            return DrawStep::done();
        }

        // Body, below the bar. When collapsed the frame already hugs the bar --
        // the placeholder lives in the field itself, so there's no body.
        if collapsed {
            return DrawStep::done();
        }
        let Some(view) = self.proj.clone() else {
            return DrawStep::done();
        };
        let field_w = rect.size.x - PAD * 2.0;

        let x = rect.pos.x + PAD;
        let mut y = rect.pos.y + BAR_H + PAD;

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
        // Type-badge colour + kind initial for the new subject.
        if let Subject::Classifier(key) = &self.subject {
            if let Some(node) = model.nodes.iter().find(|n| &n.key == key) {
                self.badge_color = bucket_color(accent_bucket(&node.ty));
            }
        }
        self.badge_letter = self
            .proj
            .as_ref()
            .and_then(|v| v.kind_label.chars().next())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        // Repointing closes an open picker; the field re-reads its selection
        // from `subject_to_index` at draw time.
        self.picker_open = false;
        self.view.redraw(cx);
    }

    /// Feed the element-picker the current diagram's rows. The owned popup reads
    /// them straight from `self.elements`. Called by `App` whenever the current
    /// diagram changes.
    pub fn set_diagram_elements(&mut self, cx: &mut Cx, rows: Vec<ElementRow>) {
        self.elements = rows;
        self.picker_open = false;
        self.view.redraw(cx);
    }

    fn open_picker(&mut self, cx: &mut Cx) {
        self.picker_open = true;
        self.picker_hover = None;
        self.view.redraw(cx);
    }

    fn close_picker(&mut self, cx: &mut Cx) {
        self.picker_open = false;
        self.picker_hover = None;
        self.view.redraw(cx);
    }

    /// Resolve a click on popup row `idx`. Node rows repoint the inspector (via
    /// an `ElementPicked` action `App` resolves); diagram/edge/placeholder rows
    /// are no-ops. Either way the list closes.
    fn choose_element(&mut self, cx: &mut Cx, idx: usize) {
        let picked = match self.elements.get(idx).map(|r| (r.kind, r.key.clone())) {
            Some((ElementKind::Node, key)) => Some(key),
            _ => None,
        };
        self.close_picker(cx);
        if let Some(key) = picked {
            cx.widget_action(self.widget_uid(), InspectorAction::ElementPicked(key));
        }
    }

    /// Draw the owned popup list below the bar: a frosted HUD sheet, then one row
    /// per element (skipping the index-0 placeholder). Association rows lead with
    /// the real `IconSpline` SDF and show only the target end; node/diagram rows
    /// align their labels to the same text column. Records each row's hit rect
    /// (paired with its `elements` index) for `handle_event`.
    fn draw_picker_list(&mut self, cx: &mut Cx2d, rect: Rect, n_rows: usize, sel: usize) {
        let sheet = Rect {
            pos: dvec2(rect.pos.x + POPUP_PAD, rect.pos.y + BAR_H),
            size: dvec2(
                (rect.size.x - POPUP_PAD * 2.0).max(0.0),
                POPUP_PAD * 2.0 + (n_rows as f64) * ROW_H_PICK,
            ),
        };
        self.draw_popup_bg.draw_abs(cx, sheet);

        let gutter_x = sheet.pos.x + POPUP_PAD;
        let text_x = gutter_x + ROW_ICON + 8.0;
        let mut y = sheet.pos.y + POPUP_PAD;
        for idx in 1..self.elements.len() {
            let row = self.elements[idx].clone();
            let row_rect = Rect {
                pos: dvec2(sheet.pos.x + 3.0, y),
                size: dvec2((sheet.size.x - 6.0).max(0.0), ROW_H_PICK),
            };
            if self.picker_hover == Some(idx) || idx == sel {
                self.draw_row_hi.draw_abs(cx, row_rect);
            }
            let text_y = y + (ROW_H_PICK - 16.0) * 0.5;
            let (label, selected): (&str, bool) = match row.kind {
                ElementKind::Edge => {
                    let icon = Rect {
                        pos: dvec2(gutter_x, y + (ROW_H_PICK - ROW_ICON) * 0.5),
                        size: dvec2(ROW_ICON, ROW_ICON),
                    };
                    self.icons.spline.color = self.draw_icon_edge.color;
                    self.icons.spline.draw_abs(cx, icon);
                    (edge_target(&row.label), idx == sel)
                }
                _ => (row.label.as_str(), idx == sel),
            };
            if selected {
                self.draw_row_sel.draw_abs(cx, dvec2(text_x, text_y), label);
            } else {
                self.draw_row_text
                    .draw_abs(cx, dvec2(text_x, text_y), label);
            }
            self.picker_rects.push((idx, row_rect));
            y += ROW_H_PICK;
        }
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

    /// Reader for a node pick from the element-picker dropdown.
    pub fn picked(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            InspectorAction::ElementPicked(key) => Some(key),
            _ => None,
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
}
