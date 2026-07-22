//! The `Inspector` widget: a right-side panel. Its **container** is a makepad
//! `View` (so it can host real child widgets — the element-picker bar, and, in
//! time, the form of editable field controls the body will grow into). The
//! **body** is still drawn immediate-mode with `DrawText`, exactly like
//! `GraphCanvas` draws node titles, until those controls actually land — the
//! same hybrid `ProjectTree` uses (derefs `View`, yet does manual draws in its
//! `draw_walk`). See `inspector.rs` for the pure `InspectorView` projection.
//!
//! Top bar: an element-picker field listing the current diagram's contents
//! (diagram, nodes, source-anchored edges), plus a square pin toggle. Clicking
//! the field emits `InspectorAction::OpenPicker`; `App` relays that to
//! `PopupRoot::show_at` (a `MenuPopup` card), and a committed pick comes back
//! through the tag-filtered `PopupRoot::closed` queue, resolved via
//! `apply_pick` (which repoints the inspector, inspector-local). Diagram/edge
//! rows are listed but picking them is a no-op for now. The pin is
//! visual-only this cut (its keep-opaque-on-blur purpose is deferred).
//!
//! Step C (inline edit): `Title`/`Description` are click-to-edit. Edits are
//! hand-rolled (no fork `TextInput`) — same convention as `doc_tabs.rs`: rects
//! captured during `draw_walk`, hit-tested on `FingerUp`, keyboard handled via
//! `cx.set_key_focus`/`Hit::KeyDown`/`Hit::TextInput`. Commits go into
//! `overrides` keyed `(subject_key, FieldId)`; the source `Model` is never
//! touched (UX mock only). A changed commit emits `InspectorAction::Edited`,
//! which `App` uses to promote the active preview tab to persisted.

use crate::icons::Icon;
use crate::icons::IconSet;
use crate::inspector::{
    build_view, effective_field, subject_to_index, ElementKind, ElementRow, FieldId, InspectorView,
    Subject, PICKER_PLACEHOLDER,
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
        // selected label) and the pencil/caret/pin glyphs -- is hand-drawn
        // immediate-mode in `draw_walk`. The dropped list itself is the shared
        // `MenuPopup` surface (routed through `PopupRoot`), so each association
        // row still carries the real `IconSpline` SDF.
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
        // Hover-translucency scrim: a window-bg quad painted over the whole panel
        // at alpha (1 - opacity), so an unhovered/unpinned panel dims toward the
        // backdrop. `atlas.ground` is the app window's background token (there is
        // no separate `atlas.bg`; `ground` is the actual app/canvas background).
        draw_scrim +: { color: atlas.ground }
        // `draw_icon_edge` is a colour-only holder whose `color` is copied onto
        // the pin/caret glyphs per draw (no RGBA crosses Rust). The element-picker
        // list itself is now drawn by the shared `MenuPopup` surface (routed
        // through `PopupRoot`), not this panel.
        draw_icon_edge +: { color: atlas.accent }
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
    }
}

/// Emitted by the inspector. `Edited` is the tab-promotion signal (`App`
/// promotes the active preview tab to persisted on receipt). `OpenPicker` is
/// the element-picker field's open-request (the inspector can't compute
/// cross-tree placement itself); `App` relays it to `PopupRoot::show_at`.
#[derive(Clone, Debug, Default)]
pub enum InspectorAction {
    #[default]
    None,
    Edited(String),
    OpenPicker {
        anchor: Rect,
        items: Vec<crate::popup::base::PopupItem>,
    },
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
    /// Hover-translucency scrim (see `draw_walk`'s `draw_hover_scrim`); painted
    /// last, over everything else, at `1 - opacity` alpha.
    #[redraw]
    #[live]
    draw_scrim: DrawColor,

    /// `draw_icon_edge` tints the pin/caret glyphs (via `icons`, the shared
    /// Atlas SDF set). The dropped list's own icons (including `IconSpline` on
    /// association rows) are drawn by `MenuPopup`, not here.
    #[live]
    draw_icon_edge: DrawColor,
    #[live]
    icons: IconSet,
    /// Left type-badge: a per-kind coloured square (`draw_badge.color` is set at
    /// draw time from the subject's `AccentBucket`) with the kind initial on top.
    #[live]
    draw_badge: DrawColor,
    #[live]
    draw_badge_text: DrawText,

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
    /// The picker field's hit rect (click target that opens the list).
    #[rust]
    picker_field_rect: Rect,
    /// id -> index into `elements`, rebuilt by `picker_items` each time the
    /// list is opened. Reverses a committed `PopupItem.id` back to its row.
    #[rust]
    picker_ids: Vec<(LiveId, usize)>,
    /// Pin toggle. Locks the hover-scrim opacity to fully opaque even when the
    /// pointer isn't over the panel (see `draw_hover_scrim`).
    #[rust]
    pinned: bool,
    #[rust]
    pin_rect: Rect,
    /// Whether the pointer is currently over the panel. Drives the hover-scrim
    /// translucency (opaque when hovered or pinned, else dimmed to 0.55).
    #[rust]
    hovered: bool,
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
const PIN_SIZE: f64 = 16.0;
const PIN_MARGIN: f64 = 14.0;
const ICON: f64 = 16.0;
const ICON_GAP: f64 = 10.0;
// Left type-badge (drawn over the field's left inset).
const BADGE_SIZE: f64 = 24.0;

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
        // All hit rects (picker field, pin, caret, pencil, inline-edit fields)
        // are recorded in `draw_walk` off `self.view.area().rect(cx)`, which
        // *during a draw* reports the pre-alignment turtle origin (x≈0). This
        // panel lives in a right-aligned parent, so the finished draw list is
        // shifted right by the panel's x — the glyphs render there, but the
        // stored rects keep the unshifted origin. Pointer events arrive in that
        // shifted (post-alignment) space, so translate the event point back
        // into draw-time space by the offset between the two before any
        // `contains` test.
        let hit_off = self.view.area().rect(cx).pos - self.view_rect.pos;
        match event.hits_with_capture_overload(cx, self.view.area(), true) {
            Hit::FingerHoverIn(_) => {
                if !self.hovered {
                    self.hovered = true;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.hovered {
                    self.hovered = false;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let p = fe.abs - hit_off;
                // Closed: the picker field opens the list.
                if self.picker_field_rect.contains(p) {
                    let screen_field = Rect {
                        pos: self.picker_field_rect.pos + hit_off,
                        size: self.picker_field_rect.size,
                    };
                    let items = self.picker_items();
                    cx.widget_action(
                        uid,
                        InspectorAction::OpenPicker {
                            anchor: screen_field,
                            items,
                        },
                    );
                    return;
                }
                if self.pin_rect.contains(p) {
                    self.pinned = !self.pinned;
                    self.view.redraw(cx);
                    return;
                }
                // Caret folds/unfolds the body (only meaningful when a subject
                // is set; with none the panel is already collapsed).
                if self.caret_rect.contains(p) {
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
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.field_rects.clear();

        let cy = rect.pos.y + BAR_H * 0.5;

        // The element-picker top bar (badge, glyph cluster, picker field, popup)
        // is diagram-only. A classifier/package preview hides it and floats the
        // body up to the panel top -- see `set_picker_visible`.
        if self.show_picker {
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
                    self.draw_badge_text.draw_abs(
                        cx,
                        dvec2(badge.pos.x + 7.0, badge.pos.y + 4.0),
                        &self.badge_letter,
                    );
                }
            }

            // Right glyph cluster, right -> left: pin, fold caret. Both are the
            // shared Atlas SDF glyphs (`icons.rs`), tinted from the panel's own
            // dim text colour via the same tint-a-shared-DrawColor idiom the edge
            // rows use below. (`draw_dim` carries the theme colour; read it out
            // before borrowing `self.icons`.)
            let dim = self.draw_dim.color;

            let pin = Rect {
                pos: dvec2(
                    rect.pos.x + rect.size.x - PIN_MARGIN - PIN_SIZE,
                    cy - PIN_SIZE * 0.5,
                ),
                size: dvec2(PIN_SIZE, PIN_SIZE),
            };
            self.pin_rect = pin;
            // Glyph carries the state (pin vs. pin-off), not colour -- both draw
            // in the neutral dim tint so the icon alone reads pinned/unpinned.
            let pin_icon = if self.pinned { Icon::Pin } else { Icon::PinOff };
            let dc = self.icons.get(pin_icon);
            dc.color = dim;
            dc.draw_abs(cx, pin);

            // Fold caret: chevrons-collapse when the body is showing (click to
            // fold), chevrons-expand when collapsed (click to unfold).
            let caret = Rect {
                pos: dvec2(pin.pos.x - ICON_GAP - ICON, cy - ICON * 0.5),
                size: dvec2(ICON, ICON),
            };
            self.caret_rect = caret;
            let caret_icon = if collapsed {
                Icon::ListExpand
            } else {
                Icon::ListCollapse
            };
            let dc = self.icons.get(caret_icon);
            dc.color = dim;
            dc.draw_abs(cx, caret);

            // Picker field label: the selected element's title (subdued,
            // text_dim), or the placeholder when nothing is picked. Sits right of
            // the badge; the whole left strip of the bar opens the list.
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
                size: dvec2((caret.pos.x - ICON_GAP - rect.pos.x).max(0.0), BAR_H),
            };
        } else {
            // No picker bar: clear its hit rects so stale rects from a previous
            // diagram tab don't catch clicks over the (now bar-less) body.
            self.picker_field_rect = Rect::default();
            self.pin_rect = Rect::default();
            self.caret_rect = Rect::default();
        }

        // Body, below the bar (or floated to the top when the bar is hidden).
        // When collapsed the frame already hugs the bar -- the placeholder lives
        // in the field itself, so there's no body.
        if collapsed {
            self.draw_hover_scrim(cx);
            return DrawStep::done();
        }
        let Some(view) = self.proj.clone() else {
            self.draw_hover_scrim(cx);
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

        self.draw_hover_scrim(cx);
        DrawStep::done()
    }
}

impl Inspector {
    /// Hover translucency (painted last, over everything): opaque panel when
    /// hovered or pinned, else dim to 0.55 via a `(1 - opacity)` backdrop
    /// scrim. Hoisted into a helper so every `draw_walk` early-return (the
    /// collapsed / empty-subject cases) still paints it before bailing.
    fn draw_hover_scrim(&mut self, cx: &mut Cx2d) {
        let opacity = if self.hovered || self.pinned {
            1.0
        } else {
            0.55
        };
        if opacity < 1.0 {
            self.draw_scrim.color.w = (1.0 - opacity) as f32;
            self.draw_scrim.draw_abs(cx, self.view_rect);
        }
    }

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
        self.view.redraw(cx);
    }

    /// Build the picker rows as `PopupItem`s and record their id→index map.
    /// Node rows are enabled (a pick repoints the inspector); edge/diagram rows
    /// are disabled (they were no-ops in the inline list). Edge labels show only
    /// the target end (as before); edges lead with the `Spline` glyph.
    fn picker_items(&mut self) -> Vec<crate::popup::base::PopupItem> {
        use crate::popup::base::PopupItem;
        self.picker_ids.clear();
        let mut items = Vec::new();
        for idx in 1..self.elements.len() {
            let row = &self.elements[idx];
            let id = LiveId::from_str(&row.key);
            self.picker_ids.push((id, idx));
            let (label, icon) = match row.kind {
                ElementKind::Edge => (edge_target(&row.label).to_string(), Icon::Spline),
                ElementKind::Node => (row.label.clone(), Icon::PackageOpen),
                _ => (row.label.clone(), Icon::SquareMenu),
            };
            items.push(PopupItem {
                id,
                label,
                icon,
                danger: false,
                enabled: matches!(row.kind, ElementKind::Node),
            });
        }
        items
    }

    /// Feed the element-picker the current diagram's rows. Called by `App`
    /// whenever the current diagram changes.
    pub fn set_diagram_elements(&mut self, cx: &mut Cx, rows: Vec<ElementRow>) {
        self.elements = rows;
        // Feeding diagram rows implies a diagram tab: show the picker bar.
        self.show_picker = true;
        self.view.redraw(cx);
    }

    /// Show/hide the element-picker top bar. Hidden while previewing a
    /// classifier/package (no diagram to pick elements from); the body then
    /// floats up to the panel top.
    pub fn set_picker_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.show_picker = visible;
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

    /// The element-picker asked to open. `App` relays this to `PopupRoot` (only
    /// the composition root can place a cross-tree popup). Anchor is the field
    /// rect in screen coords; drop the card just below it.
    pub fn open_picker_request(
        &self,
        actions: &Actions,
    ) -> Option<(Rect, Vec<crate::popup::base::PopupItem>)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let InspectorAction::OpenPicker { anchor, items } = item.cast() {
            Some((anchor, items))
        } else {
            None
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
