//! The `Inspector` widget: a right-side panel. Its **container** is a makepad
//! `View` (so it can host real child widgets — the element-picker bar, and, in
//! time, the form of editable field controls the body will grow into). The
//! **body** is still drawn immediate-mode with `DrawText`, exactly like
//! `ClassDiagramSurface` draws node titles, until those controls actually land — the
//! same hybrid `ProjectTree` uses (derefs `View`, yet does manual draws in its
//! `draw_walk`). See `inspector.rs` for the pure `InspectorView` projection.
//!
//! Top bar (`element_bar`): a real `SelectBox` child widget (badge + selected
//! label + caret) listing the current diagram's contents (diagram, nodes,
//! source-anchored edges). The panel's `dock: DockState` (Flag/Pinned; see
//! `dock.rs`) is binary now -- there is no in-panel affordance for it at all;
//! the active document header's right-dock toggle is the sole way to flip it.
//! Clicking the box opens its `SelectFlyout`
//! card (`App` relays `SelectBox`'s open request to `PopupRoot::show_at`), and
//! a committed pick comes back through the tag-filtered `PopupRoot::closed`
//! queue, resolved via `apply_pick` (which repoints the inspector,
//! inspector-local). Every listed row picks, the row-0 diagram included -- the
//! diagram is the fallback subject, selected whenever nothing else is.
//!
//! Step C (inline edit): `Title`/`Description` are click-to-edit. Edits are
//! hand-rolled (no fork `TextInput`) — same convention as `doc_tabs.rs`: rects
//! captured during `draw_walk`, hit-tested on `FingerUp`, keyboard handled via
//! `cx.set_key_focus`/`Hit::KeyDown`/`Hit::TextInput`. Commits go into
//! `overrides` keyed `(subject_key, FieldId)`; the source `Model` is never
//! touched (UX mock only). A changed commit emits `InspectorAction::Edited`,
//! which `App` uses to promote the active preview tab to persisted.

use crate::accent::bucket_color;
use crate::attr_row::AttrRowViewWidgetRefExt;
use crate::cursor;
use crate::dock::{DockEvent, DockState};
use crate::icons::{Icon, IconSet};
use crate::inspector::{
    build_view, effective_field, subject_to_index, AssocDir, AssocRow, ElementKind, ElementRow,
    FieldId, InspectorView, Subject,
};
use crate::navigation::NavigationTarget;
use crate::node_style::{accent_bucket, AccentBucket};
use crate::popup::base::PopupResult;
use crate::popup::select::{SelectItem, SelectLead};
use crate::ref_card::RefCardViewWidgetRefExt;
use crate::section_heading::SectionHeadingWidgetRefExt;
use crate::select_box::SelectBox;
use crate::tree::kind_of;
use makepad_widgets::*;
use std::collections::HashMap;
use waml::model::{ElementType, Model};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.InspectorBase = #(Inspector::register_widget(vm))

    mod.widgets.Inspector = set_type_default() do mod.widgets.InspectorBase{
        width: Fill
        height: Fill
        show_bg: true
        flow: Down
        // Flat, opaque `field_bg` -- no ring, no corner radius, no divider.
        // Mirrors `tree_panel.rs`'s own flat fill: this column is flush and
        // binary just like the tree's now, butted to the window's right edge
        // and to the caption band above it, so a ring would have nothing left
        // to separate. The two panels are symmetric on purpose -- do NOT
        // reintroduce the old AccentFrame ring here.
        draw_bg +: {
            color: atlas.field_bg
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }

        // The element-picker bar. Hosts the real `SelectBox` child widget
        // (badge + selected label + caret, its own click handling and open
        // request). Hidden (`visible: false`) here and shown by `draw_walk`
        // whenever the panel body draws; the `Flag` branch re-hides it so a
        // collapsed column costs zero pixels. The dropped list is the shared
        // `SelectFlyout` surface (routed through `PopupRoot`), so each
        // association row still carries the real `IconSpline` SDF.
        element_bar := View {
            width: Fill
            height: 56.0
            align: Align{x: 0.0, y: 0.5}
            padding: Inset{left: 16.0, right: 16.0}
            spacing: 2.0
            select_box := SelectBox { width: Fill }
        }

        // The diagram/picker body: a declared Turtle column drawn by
        // `self.view.draw_walk`, revealed only when `show_picker` (the
        // classifier-preview path keeps its own immediate-mode body). Rows
        // self-size (Fit) -- no y-offsets, no text measuring.
        //
        // `height: Fill` claims the panel space below the 56px bar so a tall
        // subject (many members/associations) scrolls instead of overflowing
        // past the frame. `scroll_bars` styled Atlas-visible: the default
        // handle color ~= our `field_bg` and vanishes (same trap tree_panel
        // dodges) -- dim ink idle, accent on hover/drag.
        body := View {
            width: Fill
            height: Fill
            flow: Down
            visible: false
            padding: Inset{left: 16.0, right: 16.0, top: 0.0, bottom: 16.0}
            spacing: 12.0
            scroll_bars: ScrollBars {
                scroll_bar_y: ScrollBar {
                    draw_bg +: {
                        color: atlas.text_dim
                        color_hover: atlas.accent
                        color_drag: atlas.accent
                    }
                }
            }

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
                    text_style: fonts.text_label
                }
            }
            // Profile row ("Profile   uml"): dim label + bright value on one
            // line. Diagram subjects only -- hidden (gap and all) for every
            // other subject, which leaves `profile` empty.
            profile_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8.0
                visible: false
                profile_label := Label {
                    text: "Profile"
                    draw_text +: {
                        color: atlas.text_dim
                        text_style: fonts.text_label
                    }
                }
                profile_value := Label {
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: atlas.text
                        text_style: fonts.text_label
                    }
                }
            }
            // Stereotype chips ("<<entity>>"): dim, label-11 (was Regular; the
            // shared token is Medium, a touch heavier than before).
            stereo := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_label
                }
            }

            attr_heading := SectionHeading { }
            // `FlatList` has neither a `#[deref]` nor a `#[visible]` field, so
            // its own `WidgetNode::set_visible` is the trait-default no-op: an
            // empty list would still walk (at 0 rows) and consume one body
            // `spacing:16` gap in the flow:Down column. Wrap it in a `View`
            // (whose `#[deref]`ed visibility does gate) so hiding the empty
            // section actually removes it -- gap and all. The id path is
            // unaffected: the draw loop finds the inner `FlatList` by id/uid
            // regardless of nesting (same idiom as `start_screen`).
            attr_list_wrap := View {
                width: Fill
                height: Fit
                flow: Down
                attr_list := FlatList {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0

                    Row := mod.widgets.AttrRowView { }
                }
            }

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
                    spacing: 6.0

                    Row := mod.widgets.RefCardView { }
                }
            }

            rel_heading := SectionHeading { }
            rel_list_wrap := View {
                width: Fill
                height: Fit
                flow: Down
                rel_list := FlatList {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0

                    Row := mod.widgets.RefCardView { }
                }
            }

            trace_heading_row := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 8.0
                visible: false
                trace_heading := SectionHeading { width: Fill }
                trace_add := Button { text: "Add" }
            }
            trace_editor := View {
                width: Fill
                height: Fit
                flow: Down
                spacing: 6.0
                visible: false
                trace_label := TextInput {
                    width: Fill
                    empty_text: "Trace label"
                }
                trace_href := TextInput {
                    width: Fill
                    empty_text: "Document, fragment, or HTTPS URL"
                }
                trace_editor_actions := View {
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 6.0
                    trace_save := Button { text: "Save" }
                    trace_cancel := Button { text: "Cancel" }
                }
            }
            trace_list_wrap := View {
                width: Fill
                height: Fit
                flow: Down
                visible: false
                trace_list := FlatList {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    Row := View {
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 4.0
                        trace_label := Label { text: "" }
                        trace_href := Label {
                            text: ""
                            draw_text +: { color: atlas.text_dim text_style: fonts.text_label }
                        }
                        trace_actions := View {
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 4.0
                            trace_open := Button { text: "Open" }
                            trace_edit := Button { text: "Edit" }
                            trace_remove := Button { text: "Remove" }
                            trace_up := Button { text: "Up" }
                            trace_down := Button { text: "Down" }
                        }
                    }
                }
            }

            desc_heading := SectionHeading { }
            // Editable description body (click-to-edit rect captured in draw_walk).
            // A wrapper View carries the edit field-bg *behind* the label so the
            // opaque fill draws under the text + cursor in a single draw_walk
            // pass (correct z-order) -- unlike the old draw_field_bg.draw_abs that
            // painted over the label afterward. The `edit` uniform (0/1) gates the
            // fill; it is pushed each frame in draw_walk.
            desc_field := View {
                width: Fill
                height: Fit
                show_bg: true
                draw_bg +: {
                    color: atlas.field_bg
                    edit: uniform(0.0)
                    pixel: fn() {
                        return vec4(self.color.x, self.color.y, self.color.z, self.color.w * self.edit)
                    }
                }
                desc := Label {
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: atlas.text
                        text_style: fonts.text_body
                    }
                }
            }
        }

        draw_title +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_label +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
        draw_field_bg +: { color: atlas.field_bg }
        // Relationship-card background: a faint field-bg fill ringed by a
        // low-alpha accent border, rounded corners via the working box-radius
        // idiom (never `sdf.box(.., 0.0)` -- floods in this fork).
        draw_card +: {
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
        // Far-end name (line 1): brighter than the dim meta run.
        draw_name +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        // Direction glyph (line 1 lead): accent-colored orientation cue. 14->13
        // collapse onto the shared heading token; watch this glyph on live
        // verification -- if the arrow degrades under the SemiBold cut, revert
        // to its own TextStyle and list it as a documented exception (Task 11).
        draw_glyph +: {
            color: atlas.accent
            text_style: fonts.text_heading
        }
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
    AddTrace {
        selector: waml::uml::TransitionSelector,
        index: usize,
        label: String,
        href: String,
    },
    UpdateTrace {
        selector: waml::uml::TransitionSelector,
        index: usize,
        label: String,
        href: String,
    },
    RemoveTrace {
        selector: waml::uml::TransitionSelector,
        index: usize,
    },
    MoveTrace {
        selector: waml::uml::TransitionSelector,
        from: usize,
        to: usize,
    },
    OpenTrace(NavigationTarget),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum TraceEditorState {
    #[default]
    Closed,
    Adding,
    Editing {
        index: usize,
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
    #[redraw]
    #[live]
    draw_card: DrawColor,
    #[redraw]
    #[live]
    draw_name: DrawText,
    #[redraw]
    #[live]
    draw_glyph: DrawText,

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
    trace_editor: TraceEditorState,
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
    /// Dock visual state (Flag / Pinned), binary like `tree_panel.rs`'s. The
    /// app reads `slot_width()` for the right slot; the active document
    /// header's right-dock toggle is the only thing that ever changes it.
    #[rust]
    dock: DockState,
    #[rust]
    presentation_visible: bool,
}

// Panel geometry (px). Fixed line advances — no text measuring in this cut.
const PAD: f64 = 16.0;
const TITLE_H: f64 = 26.0;
const ROW_H: f64 = 20.0;
const GAP: f64 = 12.0;
// Relationship-card geometry (px). No text measuring; fixed advances.
const CARD_PAD: f64 = 10.0; // inner padding of each card
const CARD_GAP: f64 = 8.0; // vertical gap between cards
const CARD_LINE_H: f64 = 18.0; // name line height (line 1)
const CARD_META_H: f64 = 16.0; // meta line height (line 2)
const CARD_H: f64 = CARD_PAD * 2.0 + CARD_LINE_H + CARD_META_H; // full card height
const GLYPH_W: f64 = 18.0; // fixed x-advance reserved for the direction glyph
                           // Bar strip height (matches `element_bar.height` in the DSL).
const BAR_H: f64 = 56.0;

// The column's open width, mirroring `tree_panel.rs`'s own slot constant --
// flush against the window edge, no `FLAG_SQUARE` tab now that the document
// header's right-dock toggle is the panel's only affordance.
pub(crate) const INSPECTOR_W: f64 = 320.0;

/// An association row's display text in the picker popup: just the target end.
/// The model label is `Source -> Target`, but each edge row is drawn beneath its
/// source node (with the `IconSpline` glyph marking it as an association), so the
/// source is redundant. Falls back to the whole label if it isn't `A -> B`.
fn edge_target(label: &str) -> &str {
    label.rsplit(" -> ").next().unwrap_or(label)
}

/// The `SelectBox` item index for `subject`. Item index == element index --
/// every element row becomes an item, the row-0 diagram included, so there is no
/// sentinel to shift past. `None` only for an empty element list, where
/// `subject_to_index`'s 0 would point past the end. Both the picker's feeders
/// (`set_diagram_elements` and `set_subject`) go through here so they cannot
/// drift apart again.
fn selected_item_index(elements: &[ElementRow], subject: &Subject) -> Option<usize> {
    (!elements.is_empty()).then(|| subject_to_index(elements, subject))
}

/// The leading orientation glyph for a relationship card. Unicode arrows
/// (\u{2192} / \u{2190} / \u{2194}); if IBM Plex Sans renders any as tofu on the
/// running native app, swap these three literals for the ASCII forms
/// `->` / `<-` / `<>` (see the verification task).
fn dir_glyph(dir: AssocDir) -> &'static str {
    match dir {
        AssocDir::Out => "\u{2192}",
        AssocDir::In => "\u{2190}",
        AssocDir::Bi => "\u{2194}",
    }
}

/// The card's second line: `kind \u{b7} role \u{b7} multiplicity`, empty parts
/// elided. Always leads with the kind so the relationship type stays scannable
/// (the mock's kind badge, folded into the meta run for v1 -- an IconSpline
/// badge is a noted follow-up).
fn meta_line(assoc: &AssocRow) -> String {
    let mut parts = vec![assoc.kind.clone()];
    if !assoc.role.is_empty() {
        parts.push(assoc.role.clone());
    }
    if !assoc.multiplicity.is_empty() {
        parts.push(assoc.multiplicity.clone());
    }
    parts.join(" \u{b7} ")
}

/// The lead glyph for a reference card, by element kind.
fn ref_card_icon(kind: ElementKind) -> Icon {
    match kind {
        ElementKind::Group => Icon::Group,
        ElementKind::Edge => Icon::Spline,
        // The container glyph, matching the diagram's own picker row.
        ElementKind::Diagram => Icon::Frame,
        ElementKind::Node => Icon::PanelTop,
        // A message/fragment is a step in a flow of control, not a container.
        ElementKind::BehaviorElement => Icon::Spline,
    }
}

/// The kind line, e.g. `Class  (abstract)`. Pure: shared by the declared body
/// column (`fill_body_column`) and the immediate-mode fallback body, which
/// duplicated it inline before.
fn kind_line(kind_label: &str, abstract_flag: bool) -> String {
    if abstract_flag {
        format!("{kind_label}  (abstract)")
    } else {
        kind_label.to_string()
    }
}

/// The stereotype chip run, e.g. `<<aggregateRoot>> <<entity>>`; empty when
/// there are no stereotypes. Pure; shared like `kind_line`.
fn stereotype_chips(stereotypes: &[String]) -> String {
    stereotypes
        .iter()
        .map(|s| format!("<<{s}>>"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The joined single-line attribute row the fallback body draws, from the same
/// `attr_line_parts` the row widget consumes.
fn attr_line(attr: &crate::inspector::AttrRow) -> String {
    let (vis, name, ty, mult) = attr_line_parts(attr);
    format!("{vis}{name}: {ty}{mult}")
}

/// Display parts for one attribute row: `(visibility, name, ty, mult)`. Empty
/// visibility and the trivial `"1"` multiplicity are elided. Kept pure so the
/// formatting is unit-tested without a `Cx`; consumed by the attribute row
/// widget (Task 3) and the interim joined line (this task).
fn attr_line_parts(attr: &crate::inspector::AttrRow) -> (String, String, String, String) {
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

/// `FlatList` item id for attribute row `i` (named `name`). Index-prefixed so
/// two attributes sharing a name (duplicate/inherited) still key to distinct
/// list items -- keying by name alone collapses them to one row
/// (last-write-wins). Mirrors the index-prefixed relationship-row keying.
fn attr_item_id(i: usize, name: &str) -> LiveId {
    LiveId::from_str(&format!("{i}-{name}"))
}

/// `FlatList` item id for member row `i` (keyed `key`). Index-prefixed so two
/// members sharing a key still key to distinct list items (mirrors
/// `attr_item_id`).
fn member_item_id(i: usize, key: &str) -> LiveId {
    LiveId::from_str(&format!("{i}-{key}"))
}

/// Leading visual for a node picker row: the shared catalog glyph for the
/// element's type when one exists, else a coloured monogram badge. Every
/// modelled UML kind resolves to an icon (`Customer`, a `Class`, leads with
/// `PanelTop` -- the same glyph the tree and doc-tab strip already draw for
/// it). An unclaimed OKF type uses the generic document glyph.
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
        if let Event::Actions(actions) = event {
            self.handle_trace_actions(cx, actions, uid);
        }
        // All hit rects (pin, caret, pencil, inline-edit fields) are recorded
        // in `draw_walk` off `self.view.area().rect(cx)`, which *during a
        // draw* reports the pre-alignment turtle origin. The panel used to live
        // in a right-aligned parent, whose alignment shifted the finished draw
        // list right of those stored rects; it is a laid-out `right_slot`
        // column now, so this offset is normally zero -- the translation is
        // kept because it stays correct either way and the rects are still
        // captured mid-draw. Pointer events arrive in post-alignment space, so
        // translate the event point back into draw-time space by the offset
        // between the two before any `contains` test. (The picker field itself
        // is now the child `SelectBox`'s own hit rect, event-time-anchored —
        // see `select_box.rs`.) `area().rect` at event time is the real
        // on-screen rect; `self.view_rect` is the draw-time rect. `hit_off` is
        // the shift between them.
        let panel_rect = self.view.area().rect(cx);
        let hit_off = panel_rect.pos - self.view_rect.pos;

        // No more in-panel dock affordances (no flag tab, no pin button, no
        // Peek auto-collapse timer) -- the active document header's right-dock
        // toggle is the panel's only way to flip `dock`. This panel is now just
        // a laid-out `right_slot` column like `tree_panel.rs`'s.
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
            // Only the field rects are click-to-edit; the rest of the column is
            // read-only text, so the cursor tracks the pointer WITHIN the panel
            // (`HoverOver`), not just its entry.
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let p = fe.abs - hit_off;
                if self.field_rects.iter().any(|(_, rect)| rect.contains(p)) {
                    cursor::hover_in(cx, MouseCursor::Text);
                } else {
                    cursor::hover_out(cx);
                }
            }
            Hit::FingerHoverOut(_) => cursor::hover_out(cx),
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
        // Flag rest state: zero pixels, nothing drawn -- mirrors
        // `tree_panel.rs`'s own Flag branch exactly.
        if !self.presentation_visible {
            let mut fw = walk;
            fw.width = Size::Fixed(0.0);
            fw.height = Size::Fixed(0.0);
            fw.margin = Inset::default();
            self.view.view(cx, ids!(element_bar)).set_visible(cx, false);
            self.view.widget(cx, ids!(body)).set_visible(cx, false);
            // `View::draw_walk` is a multi-step `DrawStep` machine: it opens the
            // view's turtle on the first call and only closes it once the loop
            // runs it to `done`. Calling it once and dropping the result leaves
            // the turtle begun-but-never-ended, unbalancing the whole window's
            // turtle stack -- every later draw (the sibling panel and the window
            // caption/frame) then silently aborts. Drive it to completion,
            // exactly like the expanded path below.
            while self.view.draw_walk(cx, scope, fw).step().is_some() {}
            return DrawStep::done();
        }
        self.view.view(cx, ids!(element_bar)).set_visible(cx, true);
        // Draw the container (HUD frame bg) and the bar child (dropdown).
        // `collapsed` = nothing selected: the frame still fills full height (like
        // the left Model panel), but the body content below the bar is suppressed.
        let collapsed = self.proj.is_none();
        // When the presentation is visible, draw a full-height column flush to the window edge --
        // the DSL `height: Fill` carries through untouched, even with nothing
        // selected. Strip the docked-edge (right) margin + the top/bottom
        // margins so no window-bg frame shows; `right_slot` reserves the
        // matching sampled width (see `slot_width`).
        let mut walk = walk;
        walk.margin.right = 0.0;
        walk.margin.top = 0.0;
        walk.margin.bottom = 0.0;
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
            // Gate the description edit field-bg: `1.0` fills the wrapper behind
            // the label while editing (drawn under the text/cursor), `0.0` leaves
            // it transparent. Must be set before the wrapper draws below.
            let editing_desc = self.editing == Some(FieldId::Description);
            if let Some(mut v) = self.view.view(cx, ids!(body.desc_field)).borrow_mut() {
                v.draw_bg
                    .set_uniform(cx, live_id!(edit), &[if editing_desc { 1.0 } else { 0.0 }]);
            }
        }

        let attr_list_uid = self.view.widget(cx, ids!(body.attr_list)).widget_uid();
        let members_list_uid = self.view.widget(cx, ids!(body.members_list)).widget_uid();
        let rel_list_uid = self.view.widget(cx, ids!(body.rel_list)).widget_uid();
        let trace_list_uid = self.view.widget(cx, ids!(body.trace_list)).widget_uid();
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if !show_body {
                continue;
            }
            if item.widget_uid() == attr_list_uid {
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
                            rv.set_target(&m.key, m.kind);
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
                            rv.set_meta(
                                cx,
                                &format!("{} {}", dir_glyph(assoc.dir), meta_line(assoc)),
                            );
                            rv.set_target(&assoc.target_key, assoc.target_kind);
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
            if item.widget_uid() == trace_list_uid {
                if let Some(view) = self.proj.clone() {
                    if let Some(mut list) = item.as_flat_list().borrow_mut() {
                        for (index, trace) in view.traces.iter().enumerate() {
                            let item_id = LiveId::from_str(&format!("trace-{index}"));
                            let row = list.item(cx, item_id, id!(Row)).unwrap();
                            row.label(cx, ids!(trace_label)).set_text(cx, &trace.label);
                            row.label(cx, ids!(trace_href)).set_text(cx, &trace.href);
                            row.widget(cx, ids!(trace_open))
                                .set_visible(cx, trace.navigation.is_some());
                            row.widget(cx, ids!(trace_up)).set_visible(cx, index > 0);
                            row.widget(cx, ids!(trace_down))
                                .set_visible(cx, index + 1 < view.traces.len());
                            row.draw_all(cx, &mut Scope::empty());
                        }
                    }
                }
            }
        }

        let rect = self.view.area().rect(cx);
        self.view_rect = rect;
        self.field_rects.clear();

        // Body, below the bar. When nothing is selected the full-height frame
        // stays empty below the bar (no field_rects to capture) -- the picker bar
        // alone sits at the top of the column.
        if collapsed {
            return DrawStep::done();
        }

        if self.show_picker {
            // Capture the description Label's drawn rect as the click-to-edit
            // target. The edit field-bg is drawn declaratively by the `desc_field`
            // wrapper (behind the label), so nothing is painted over it here.
            let desc_rect = self
                .view
                .label(cx, ids!(body.desc_field.desc))
                .area()
                .rect(cx);
            self.field_rects.push((FieldId::Description, desc_rect));
            return DrawStep::done();
        }

        // ---- `show_picker == false`: baseline immediate-mode body (out of
        // scope, unchanged). ----
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
        self.draw_dim.draw_abs(
            cx,
            dvec2(x, y),
            &kind_line(&view.kind_label, view.abstract_flag),
        );
        y += ROW_H;

        // Stereotype chips, e.g. "<<aggregateRoot>> <<entity>>". Read-only breadth (U6).
        if !view.stereotypes.is_empty() {
            self.draw_dim
                .draw_abs(cx, dvec2(x, y), &stereotype_chips(&view.stereotypes));
            y += ROW_H;
        }
        y += GAP;

        if !view.attributes.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "ATTRIBUTES");
            y += ROW_H;
            for attr in &view.attributes {
                self.draw_label.draw_abs(cx, dvec2(x, y), &attr_line(attr));
                y += ROW_H;
            }
            y += GAP;
        }

        // Members: a group's direct members, one dim row each. Mirrors the
        // ATTRIBUTES compartment; only groups populate `view.members`.
        if !view.members.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "MEMBERS");
            y += ROW_H;
            for m in &view.members {
                self.draw_label.draw_abs(cx, dvec2(x, y), &m.label);
                y += ROW_H;
            }
            y += GAP;
        }

        // Relationships: read-only cards derived from Model::edges (U6 breadth).
        // One bordered rounded-rect card per edge: line 1 = direction glyph
        // (accent) + far-end name (bright); line 2 = a dim meta run
        // "kind \u{b7} role \u{b7} multiplicity" (empty parts elided). Not
        // click-to-edit -- no single scalar override target for a relationship.
        if !view.associations.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "RELATIONSHIPS");
            y += ROW_H;
            for assoc in &view.associations {
                let card = Rect {
                    pos: dvec2(x, y),
                    size: dvec2(field_w, CARD_H),
                };
                self.draw_card.draw_abs(cx, card);
                let gx = x + CARD_PAD;
                let gy = y + CARD_PAD;
                self.draw_glyph
                    .draw_abs(cx, dvec2(gx, gy), dir_glyph(assoc.dir));
                self.draw_name
                    .draw_abs(cx, dvec2(gx + GLYPH_W, gy), &assoc.other_label);
                self.draw_dim
                    .draw_abs(cx, dvec2(gx, gy + CARD_LINE_H), &meta_line(assoc));
                y += CARD_H + CARD_GAP;
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
    fn begin_trace_edit(&mut self, cx: &mut Cx, state: TraceEditorState, label: &str, href: &str) {
        self.trace_editor = state;
        self.view
            .text_input(cx, ids!(body.trace_editor.trace_label))
            .set_text(cx, label);
        self.view
            .text_input(cx, ids!(body.trace_editor.trace_href))
            .set_text(cx, href);
        self.view.redraw(cx);
    }

    fn handle_trace_actions(&mut self, cx: &mut Cx, actions: &Actions, uid: WidgetUid) {
        let Some(view) = self.proj.clone() else {
            return;
        };
        let Some(selector) = view.transition_selector.clone() else {
            return;
        };
        if self
            .view
            .button(cx, ids!(body.trace_heading_row.trace_add))
            .clicked(actions)
        {
            self.begin_trace_edit(cx, TraceEditorState::Adding, "", "");
            return;
        }
        if self
            .view
            .button(
                cx,
                ids!(body.trace_editor.trace_editor_actions.trace_cancel),
            )
            .clicked(actions)
        {
            self.trace_editor = TraceEditorState::Closed;
            self.view.redraw(cx);
            return;
        }
        if self
            .view
            .button(cx, ids!(body.trace_editor.trace_editor_actions.trace_save))
            .clicked(actions)
        {
            let label = self
                .view
                .text_input(cx, ids!(body.trace_editor.trace_label))
                .text();
            let href = self
                .view
                .text_input(cx, ids!(body.trace_editor.trace_href))
                .text();
            if label.trim().is_empty() || href.trim().is_empty() {
                return;
            }
            let action = match self.trace_editor.clone() {
                TraceEditorState::Adding => Some(InspectorAction::AddTrace {
                    selector,
                    index: view.traces.len(),
                    label,
                    href,
                }),
                TraceEditorState::Editing { index } => Some(InspectorAction::UpdateTrace {
                    selector,
                    index,
                    label,
                    href,
                }),
                TraceEditorState::Closed => None,
            };
            if let Some(action) = action {
                self.trace_editor = TraceEditorState::Closed;
                cx.widget_action(uid, action);
                self.view.redraw(cx);
            }
            return;
        }

        let list = self.view.flat_list(cx, ids!(body.trace_list));
        for (item_id, item) in list.items_with_actions(actions) {
            let Some(index) = view.traces.iter().enumerate().find_map(|(index, _)| {
                (item_id == LiveId::from_str(&format!("trace-{index}"))).then_some(index)
            }) else {
                continue;
            };
            if item.button(cx, ids!(trace_open)).clicked(actions) {
                if let Some(target) = view.traces[index].navigation.clone() {
                    cx.widget_action(uid, InspectorAction::OpenTrace(target));
                }
            } else if item.button(cx, ids!(trace_edit)).clicked(actions) {
                self.begin_trace_edit(
                    cx,
                    TraceEditorState::Editing { index },
                    &view.traces[index].label,
                    &view.traces[index].href,
                );
            } else if item.button(cx, ids!(trace_remove)).clicked(actions) {
                cx.widget_action(
                    uid,
                    InspectorAction::RemoveTrace {
                        selector: selector.clone(),
                        index,
                    },
                );
            } else if item.button(cx, ids!(trace_up)).clicked(actions) && index > 0 {
                cx.widget_action(
                    uid,
                    InspectorAction::MoveTrace {
                        selector: selector.clone(),
                        from: index,
                        to: index - 1,
                    },
                );
            } else if item.button(cx, ids!(trace_down)).clicked(actions)
                && index + 1 < view.traces.len()
            {
                cx.widget_action(
                    uid,
                    InspectorAction::MoveTrace {
                        selector: selector.clone(),
                        from: index,
                        to: index + 1,
                    },
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn elements_for_test(&self) -> &[ElementRow] {
        &self.elements
    }

    /// Push the current projection into the declared `body` column widgets
    /// (kind, stereotypes, the three headings, the interim attribute/relationship
    /// text, and the description). Hides a heading + its rows when that section
    /// is empty. Called each `draw_walk` for the `show_picker` path.
    fn fill_body_column(&mut self, cx: &mut Cx, view: &InspectorView) {
        self.view
            .label(cx, ids!(body.kind))
            .set_text(cx, &kind_line(&view.kind_label, view.abstract_flag));

        // Profile: diagram subjects only; the row (label included) disappears
        // when empty, so no stray gap for a classifier/group/edge.
        self.view
            .widget(cx, ids!(body.profile_row))
            .set_visible(cx, !view.profile.is_empty());
        self.view
            .label(cx, ids!(body.profile_row.profile_value))
            .set_text(cx, &view.profile);

        let stereo = stereotype_chips(&view.stereotypes);
        self.view
            .widget(cx, ids!(body.stereo))
            .set_visible(cx, !stereo.is_empty());
        self.view.label(cx, ids!(body.stereo)).set_text(cx, &stereo);

        let has_attrs = !view.attributes.is_empty();
        self.view
            .widget(cx, ids!(body.attr_heading))
            .set_visible(cx, has_attrs);
        // Gate the wrapper `View` (not the `FlatList`, whose `set_visible` is a
        // no-op) so an empty section leaves no present-but-0-row list and no
        // stray body gap.
        self.view
            .widget(cx, ids!(body.attr_list_wrap))
            .set_visible(cx, has_attrs);
        if has_attrs {
            self.view
                .widget(cx, ids!(body.attr_heading))
                .as_section_heading()
                .set_text(cx, "ATTRIBUTES");
        }

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

        let has_rels = !view.associations.is_empty();
        self.view
            .widget(cx, ids!(body.rel_heading))
            .set_visible(cx, has_rels);
        // Gate the wrapper `View`, not the `FlatList` (see `attr_list_wrap`).
        self.view
            .widget(cx, ids!(body.rel_list_wrap))
            .set_visible(cx, has_rels);
        if has_rels {
            self.view
                .widget(cx, ids!(body.rel_heading))
                .as_section_heading()
                .set_text(cx, "RELATIONSHIPS");
        }

        let has_transition = view.transition_selector.is_some();
        self.view
            .widget(cx, ids!(body.trace_heading_row))
            .set_visible(cx, has_transition);
        self.view
            .widget(cx, ids!(body.trace_list_wrap))
            .set_visible(cx, has_transition && !view.traces.is_empty());
        self.view.widget(cx, ids!(body.trace_editor)).set_visible(
            cx,
            has_transition && self.trace_editor != TraceEditorState::Closed,
        );
        if has_transition {
            self.view
                .widget(cx, ids!(body.trace_heading_row.trace_heading))
                .as_section_heading()
                .set_text(cx, "TRACES");
        }

        // DESCRIPTION (always shown so there is an affordance to add one).
        self.view
            .widget(cx, ids!(body.desc_heading))
            .as_section_heading()
            .set_text(cx, "DESCRIPTION");
        let desc_text = if self.editing == Some(FieldId::Description) {
            format!("{}\u{2502}", self.edit_buffer)
        } else {
            let t = self.effective_description(view);
            if t.is_empty() {
                "(click to add)".to_string()
            } else {
                t
            }
        };
        self.view
            .label(cx, ids!(body.desc_field.desc))
            .set_text(cx, &desc_text);
    }

    /// Point the inspector at `subject`, rebuilding the projection and syncing
    /// the picker's selected row. Overrides persist across subject switches
    /// (keyed per subject); an in-progress edit is discarded uncommitted.
    pub fn set_subject(&mut self, cx: &mut Cx, model: &Model, subject: Subject) {
        self.proj = build_view(model, &subject);
        self.install_subject(cx, subject);
    }

    pub fn set_subject_analysis(
        &mut self,
        cx: &mut Cx,
        analysis: &waml::uml::Analysis,
        subject: Subject,
    ) {
        self.proj = crate::inspector::build_view_from_analysis(analysis, &subject);
        self.install_subject(cx, subject);
    }

    fn install_subject(&mut self, cx: &mut Cx, subject: Subject) {
        self.subject = subject;
        self.editing = None;
        self.trace_editor = TraceEditorState::Closed;
        // Subject changes no longer force-unfold -- dock state is independent
        // of the selected subject.
        // Re-mark the box's selection so a pick made elsewhere (canvas/tree)
        // shows up in the picker bar too.
        let sel_in_items = selected_item_index(&self.elements, &self.subject);
        if let Some(mut b) = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
        {
            b.set_selected(cx, sel_in_items);
        }
        self.view.redraw(cx);
    }

    /// Apply a dock event: transition + redraw. No-op if the state is
    /// unchanged. Mirrors `ProjectTree::apply_dock` exactly.
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        if crate::dock::apply(&mut self.dock, ev) {
            self.view.redraw(cx);
        }
    }

    /// Expand <-> collapse, driven by the active document header's right-dock
    /// toggle. Binary by construction: `DockEvent::Toggle` never routes through
    /// `Peek`, so the column is either a full 320px or zero pixels.
    pub fn toggle_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Toggle);
    }

    /// Open the panel idempotently for responsive shell coordination. A no-op
    /// when already open, so repeated layout reconciliation causes no redraw.
    pub fn open_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Open);
    }

    /// Force the panel shut, idempotently -- the shell's reconcile for an active
    /// view that declares no right dock (`BodyChrome.right_dock == None`; see
    /// `BodyWidgets::apply_chrome`). Never expands: see `DockEvent::Close`. A
    /// no-op when already closed, so there is no redraw churn on tab switches.
    pub fn close_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Close);
    }

    pub fn set_presentation_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.presentation_visible == visible {
            return;
        }
        self.presentation_visible = visible;
        self.view.redraw(cx);
    }

    /// The current dock state. Plan-specified symmetry accessor (mirrors
    /// `ProjectTree::dock_state`); no in-crate caller yet since the app
    /// currently only reads `slot_width()`.
    #[allow(dead_code)]
    pub fn dock_state(&self) -> DockState {
        self.dock
    }

    /// The layout width the app must reserve in the right slot for this panel.
    #[allow(dead_code)]
    pub fn slot_width(&self) -> f64 {
        crate::dock::slot_width(self.dock, INSPECTOR_W)
    }

    pub fn drawn_rect(&self, cx: &Cx) -> Rect {
        self.view.area().rect(cx)
    }

    /// Build the picker rows as `SelectItem`s and record their id→index map (for
    /// `apply_pick`). Node rows lead with their catalog glyph (see `node_lead`,
    /// falling back to a per-type badge for `Unknown` types) and are enabled;
    /// group rows lead with the dashed-box glyph and are enabled; edge rows
    /// lead with the spline glyph (target-end label) and are enabled; the row-0
    /// diagram row leads with the `Frame` glyph and is enabled too (it is the
    /// fallback subject). Every row maps 1:1 to an item -- there is no skipped
    /// sentinel, so item index == element index.
    fn build_select_items(&mut self, model: &Model) -> Vec<SelectItem> {
        self.picker_ids.clear();
        let mut items = Vec::new();
        for idx in 0..self.elements.len() {
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
                ElementKind::Group => (
                    // The Lucide group glyph -- distinct from the diagram's solid
                    // `Frame` and any node's catalog icon. Drives the collapsed
                    // select-box lead (the panel header) too.
                    SelectLead::Icon(Icon::Group),
                    row.label.clone(),
                    true,
                ),
                ElementKind::Edge => (
                    SelectLead::Icon(Icon::Spline),
                    edge_target(&row.label).to_string(),
                    true,
                ),
                // The root diagram row leads with the `Frame` glyph -- distinct
                // from any node's catalog icon, marking it as the container.
                ElementKind::Diagram => (SelectLead::Icon(Icon::Frame), row.label.clone(), true),
                // A behavior element's label is already its full reading (`a
                // calls b `start()`), so it goes through verbatim -- unlike an
                // edge row, whose label is a `src -> tgt` pair the picker trims
                // to the target end.
                ElementKind::BehaviorElement => {
                    (SelectLead::Icon(Icon::Spline), row.label.clone(), true)
                }
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
        let sel_in_items = selected_item_index(&self.elements, &self.subject);
        if let Some(mut b) = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
        {
            b.set_items(cx, items);
            b.set_selected(cx, sel_in_items);
        }
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
    /// the id isn't in the current element list at all. Every listed row is
    /// pickable, the diagram row included.
    pub fn apply_pick(
        &mut self,
        cx: &mut Cx,
        analysis: &waml::uml::Analysis,
        id: LiveId,
    ) -> Option<Subject> {
        let idx = self
            .picker_ids
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, x)| *x)?;
        let row = self.elements.get(idx)?;
        let subject = crate::inspector::subject_from(&row.key, row.kind);
        self.set_subject_analysis(cx, analysis, subject.clone());
        Some(subject)
    }

    fn subject_key(&self) -> Option<String> {
        match &self.subject {
            Subject::Diagram(key)
            | Subject::Classifier(key)
            | Subject::Group(key)
            | Subject::Edge(key)
            | Subject::BehaviorElement(key) => Some(key.clone()),
            Subject::None => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn subject_key_for_test(&self) -> Option<String> {
        self.subject_key()
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

    pub fn trace_action(&self, actions: &Actions) -> Option<InspectorAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            action @ (InspectorAction::AddTrace { .. }
            | InspectorAction::UpdateTrace { .. }
            | InspectorAction::RemoveTrace { .. }
            | InspectorAction::MoveTrace { .. }
            | InspectorAction::OpenTrace(_)) => Some(action),
            InspectorAction::None | InspectorAction::Edited(_) => None,
        }
    }

    /// A member/association card was clicked this pass. Scans both section
    /// FlatLists' grouped actions and returns the clicked row's `(key, kind)`.
    /// `App`/`ClassDiagramView` repoints the inspector and selects the node.
    pub fn navigate(&mut self, cx: &mut Cx, actions: &Actions) -> Option<(String, ElementKind)> {
        let members = self.view.flat_list(cx, ids!(body.members_list));
        for (_item_id, item) in members.items_with_actions(actions) {
            if let Some(t) = item.as_ref_card_view().nav_target(actions) {
                return Some(t);
            }
        }
        let rel = self.view.flat_list(cx, ids!(body.rel_list));
        for (_item_id, item) in rel.items_with_actions(actions) {
            if let Some(t) = item.as_ref_card_view().nav_target(actions) {
                return Some(t);
            }
        }
        None
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
    /// repoint the inspector via `apply_pick`. Returns the new subject on a
    /// commit, so a caller that also owns a canvas can follow the pick there
    /// (`None` on a dismiss, or on an id with no matching row).
    pub fn on_picker_closed(
        &mut self,
        cx: &mut Cx,
        analysis: &waml::uml::Analysis,
        result: PopupResult,
    ) -> Option<Subject> {
        let picked = self
            .view
            .widget(cx, ids!(element_bar.select_box))
            .borrow_mut::<SelectBox>()
            .and_then(|mut b| b.on_closed(cx, result))?;
        self.apply_pick(cx, analysis, picked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_subject_installs_trace_section_and_editor_fields() {
        let mut vm = crate::script_gate::boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);
        crate::fonts::script_mod(&mut vm);
        crate::icons::script_mod(&mut vm);
        crate::icon_button::script_mod(&mut vm);
        crate::select_box::script_mod(&mut vm);
        crate::section_heading::script_mod(&mut vm);
        crate::attr_row::script_mod(&mut vm);
        crate::ref_card::script_mod(&mut vm);
        crate::inspector_panel::script_mod(&mut vm);
        let value = script_eval!(vm, { mod.widgets.Inspector {} });
        let mut widget = WidgetRef::script_new(&mut vm);
        widget.script_apply(&mut vm, &Apply::New, &mut Scope::empty(), value);
        let cx = vm.cx_mut();
        let mut inspector = widget.borrow_mut::<Inspector>().unwrap();
        let source = waml::source::SourceBundle::try_from_pairs([(
            "flow.md",
            "---\ntype: uml.Activity\ntitle: Flow\n---\n# Flow\n\n## Nodes\n\n### Start\n- transitions to Done traces [Claim](https://example.com/claim)\n\n### Done\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let edge_key = prepared.uml().projection.flow_edges[0].key.clone();

        inspector.set_subject_analysis(cx, prepared.uml(), Subject::Edge(edge_key));
        assert_eq!(inspector.proj.as_ref().unwrap().traces.len(), 1);
        inspector.begin_trace_edit(
            cx,
            TraceEditorState::Editing { index: 0 },
            "Updated",
            "https://example.com/updated",
        );

        assert_eq!(
            inspector.trace_editor,
            TraceEditorState::Editing { index: 0 }
        );
        assert!(inspector
            .view
            .widget(cx, ids!(body.trace_heading_row))
            .borrow::<View>()
            .is_some());
        assert!(inspector
            .view
            .widget(cx, ids!(body.trace_editor))
            .borrow::<View>()
            .is_some());
        assert!(inspector
            .view
            .widget(cx, ids!(body.trace_editor.trace_label))
            .borrow::<TextInput>()
            .is_some());
        assert_eq!(
            inspector
                .view
                .text_input(cx, ids!(body.trace_editor.trace_label))
                .text(),
            "Updated"
        );
        assert_eq!(
            inspector
                .view
                .text_input(cx, ids!(body.trace_editor.trace_href))
                .text(),
            "https://example.com/updated"
        );
    }

    #[test]
    fn picker_selection_keeps_declared_recovery_rows_and_revision_bound_actions() {
        let mut vm = crate::script_gate::boot_test_vm();
        let mut inspector = Inspector::script_new(&mut vm);
        let cx = vm.cx_mut();
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "diagram.md",
                "---\ntype: Diagram\nprofile: uml-domain\n---\n# Diagram\n\n## Members\n- [Broken](./broken.md)\n",
            ),
            (
                "broken.md",
                "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- name String [oops 42]\n",
            ),
        ])
        .unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session.replace(source).unwrap();
        let analysis = session.uml_analysis();
        let model = &analysis.projection;
        let rows =
            crate::inspector::diagram_elements(model, "diagram", "Diagram", &["broken".into()]);
        inspector.set_diagram_elements(cx, model, rows);

        assert_eq!(
            inspector.apply_pick(cx, analysis, LiveId::from_str("broken")),
            Some(Subject::Classifier("broken".into()))
        );
        let projection = inspector.proj.as_ref().unwrap();
        assert_eq!(projection.attributes.len(), 1);
        assert_eq!(
            projection.attributes[0].multiplicity,
            "<invalid multiplicity>"
        );

        let snapshot = session.snapshot();
        let mut actions: Vec<_> = crate::doc_view::ViewData::from(snapshot.borrowed())
            .uml_repair_actions("broken")
            .unwrap();
        let titles: Vec<_> = actions.iter().map(|action| action.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Insert missing `: `", "Replace invalid multiplicity"]
        );
        let first = actions.remove(0);
        let stale = actions.remove(0);
        session.apply(first.edit).unwrap();
        assert!(session.apply(stale.edit).is_err());
    }

    fn rows() -> Vec<ElementRow> {
        vec![
            ElementRow {
                key: "d1".into(),
                label: "Orders".into(),
                kind: ElementKind::Diagram,
            },
            ElementRow {
                key: "Customer".into(),
                label: "Customer".into(),
                kind: ElementKind::Node,
            },
            ElementRow {
                key: "Order".into(),
                label: "Order".into(),
                kind: ElementKind::Node,
            },
        ]
    }

    // The box label is drawn from `items[selected]`, so a selection that is off
    // by one shows the PREVIOUS row's name over the right body -- the sentinel
    // row is gone, and with it the shift that used to compensate for it.
    #[test]
    fn selected_item_index_does_not_shift_past_a_sentinel() {
        let rows = rows();
        assert_eq!(
            selected_item_index(&rows, &Subject::Classifier("Order".into())),
            Some(2)
        );
    }

    // Row 0 is the diagram and is pickable; answering `None` here would blank
    // the box instead of naming the diagram.
    #[test]
    fn selected_item_index_resolves_the_row_zero_diagram() {
        let rows = rows();
        assert_eq!(
            selected_item_index(&rows, &Subject::Diagram("d1".into())),
            Some(0)
        );
        // `Subject::None` falls back to the diagram too.
        assert_eq!(selected_item_index(&rows, &Subject::None), Some(0));
    }

    #[test]
    fn selected_item_index_is_none_for_an_empty_list() {
        assert_eq!(selected_item_index(&[], &Subject::None), None);
    }

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

    // Unclaimed OKF types use the same generic-document glyph as the tree.
    #[test]
    fn node_lead_uses_generic_okf_icon_for_unclaimed_type() {
        let lead = node_lead(&ElementType::Unknown("Widget".into()), "W".into());
        assert!(matches!(lead, SelectLead::Icon(Icon::FileText)));
    }

    #[test]
    fn kind_line_appends_the_abstract_badge_only_when_flagged() {
        assert_eq!(kind_line("Class", true), "Class  (abstract)");
        assert_eq!(kind_line("Class", false), "Class");
    }

    #[test]
    fn stereotype_chips_wrap_and_join_with_spaces() {
        assert_eq!(
            stereotype_chips(&["aggregateRoot".into(), "entity".into()]),
            "<<aggregateRoot>> <<entity>>"
        );
        assert_eq!(stereotype_chips(&[]), "");
    }

    #[test]
    fn attr_line_joins_the_parts_the_row_widget_consumes() {
        let a = crate::inspector::AttrRow {
            name: "items".into(),
            ty: "Product".into(),
            multiplicity: "0..*".into(),
            visibility: "+".into(),
        };
        assert_eq!(attr_line(&a), "+ items: Product  [0..*]");
    }

    #[test]
    fn dir_glyph_maps_each_orientation() {
        assert_eq!(dir_glyph(AssocDir::Out), "\u{2192}");
        assert_eq!(dir_glyph(AssocDir::In), "\u{2190}");
        assert_eq!(dir_glyph(AssocDir::Bi), "\u{2194}");
    }

    #[test]
    fn meta_line_joins_present_parts_with_middot() {
        let assoc = AssocRow {
            kind: "associates".into(),
            dir: AssocDir::Out,
            other_label: "Customer".into(),
            role: "buyer".into(),
            multiplicity: "0..1".into(),
            target_key: "customer".into(),
            target_kind: ElementKind::Node,
        };
        assert_eq!(meta_line(&assoc), "associates \u{b7} buyer \u{b7} 0..1");
    }

    #[test]
    fn meta_line_elides_empty_role_and_multiplicity() {
        let assoc = AssocRow {
            kind: "associates".into(),
            dir: AssocDir::In,
            other_label: "Order".into(),
            role: String::new(),
            multiplicity: String::new(),
            target_key: "order".into(),
            target_kind: ElementKind::Node,
        };
        assert_eq!(meta_line(&assoc), "associates");
    }

    // Two attributes sharing a name must key to *distinct* FlatList items
    // (index-prefixed), else the list collapses them to one row and a class with
    // duplicate/inherited attribute names renders fewer/overlapping rows.
    #[test]
    fn attr_item_id_distinguishes_duplicate_names() {
        assert_ne!(attr_item_id(0, "id"), attr_item_id(1, "id"));
    }

    // Same index + same name is stable (the row keeps its identity across draws).
    #[test]
    fn attr_item_id_is_stable_for_same_index_and_name() {
        assert_eq!(attr_item_id(2, "name"), attr_item_id(2, "name"));
    }

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
        assert!(matches!(ref_card_icon(ElementKind::Group), Icon::Group));
    }

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
            (
                "+ ".into(),
                "items".into(),
                "Product".into(),
                "  [0..*]".into()
            )
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

    // The document header's right-dock toggle is the panel's only affordance
    // now, so the state machine it drives must be strictly binary -- landing in
    // `Peek` would self-collapse the column out from under the user.
    #[test]
    fn the_caption_toggle_moves_the_column_between_flag_and_pinned() {
        use crate::dock::{next, DockEvent, DockState};
        assert_eq!(next(DockState::Flag, DockEvent::Toggle), DockState::Pinned);
        assert_eq!(next(DockState::Pinned, DockEvent::Toggle), DockState::Flag);
        assert_ne!(next(DockState::Flag, DockEvent::Toggle), DockState::Peek);
        assert_ne!(next(DockState::Pinned, DockEvent::Toggle), DockState::Peek);
    }

    #[test]
    fn the_inspector_column_reserves_320_open_and_nothing_closed() {
        use crate::dock::{slot_width, DockState};
        assert_eq!(slot_width(DockState::Pinned, INSPECTOR_W), 320.0);
        assert_eq!(slot_width(DockState::Flag, INSPECTOR_W), 0.0);
    }

    // `DockState::default()` is spelled `Flag` on the enum precisely because
    // the inspector depends on it (the tree seeds its own `Pinned` at the
    // field). The inspector still wants to start collapsed.
    #[test]
    fn the_inspector_starts_collapsed() {
        use crate::dock::DockState;
        assert_eq!(DockState::default(), DockState::Flag);
    }

    #[test]
    fn presentation_visibility_keeps_the_inspector_drawable_after_its_dock_closes() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let mut panel = cx.with_vm(Inspector::script_new_with_default);

        panel.set_presentation_visible(&mut cx, true);

        assert_eq!(panel.dock_state(), crate::dock::DockState::Flag);
        assert!(panel.presentation_visible);
    }
}
