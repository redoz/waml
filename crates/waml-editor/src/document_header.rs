use crate::cursor;
use crate::doc_view::HeaderViewAction;
use crate::icon_button::{IconButtonAction, IconButtonWidgetRefExt};
use crate::icons::Icon;
use crate::navigation::{BreadcrumbSegment, NavigationTarget};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.DocumentHeaderBase = #(DocumentHeader::register_widget(vm))

    // Private Lucide `chevron-right` port. The shared public Icon enum does not
    // expose this glyph, and a breadcrumb separator should not widen that API.
    mod.draw.DocumentHeaderChevron = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = max(1.0, s * 0.125)
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.375, s * 0.25)
            sdf.line_to(s * 0.625, s * 0.5)
            sdf.line_to(s * 0.375, s * 0.75)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    mod.widgets.DocumentHeader = set_type_default() do mod.widgets.DocumentHeaderBase{
        width: Fill
        height: 0.0
        flow: Overlay
        clip_x: true

        // The crumb sits UNDER the doc tabs, so it must never out-size them:
        // tabs are `fonts.text_menu` (10 Regular), so the current segment takes
        // the same 10 in Medium and the ancestors drop to 9.
        draw_ancestor +: {
            // `text_mid`, not `text_dim`: at 9px italic the dim grey falls under
            // the surface contrast floor and the ancestors read as disabled.
            color: atlas.text_mid
            text_style: fonts.text_micro_italic
        }
        draw_current +: {
            color: atlas.text
            text_style: fonts.text_compact_label_italic
        }
        draw_border +: { color: atlas.surface_border }
        draw_separator: mod.draw.DocumentHeaderChevron{ color: atlas.text }

        surface := View {
            width: Fill
            height: Fill
            show_bg: true
            draw_bg +: {
                color: atlas.canvas_ground
                pixel: fn() {
                    return vec4(self.color.rgb * self.color.a, self.color.a)
                }
            }
        }
        content_row := View {
            width: Fill
            height: Fill
            flow: Right
            align: Align{y: 0.5}
            clip_x: true

            breadcrumb_slot := View {
                width: Fill
                height: Fill
            }
            // Trailing action buttons. `view_button` is the active document's
            // view action; `right_button` is the inspector dock toggle. More
            // document actions slot in here.
            view_button := IconButton {
                visible: false
                width: 30.0
                height: 30.0
            }
            right_button := IconButton {
                visible: false
                width: 30.0
                height: 30.0
            }
        }
    }
}

pub const DOCUMENT_HEADER_H: f64 = 30.0;
const HEADER_PAD_X: f64 = 8.0;
const SEGMENT_PAD_X: f64 = 6.0;
const SEPARATOR_SLOT_W: f64 = 12.0;
const SEPARATOR_SIZE: f64 = 10.0;
const TEXT_DY: f64 = 1.0;

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentHeaderAction {
    RevealInTree(NavigationTarget),
    ToggleRightDock,
}

pub struct DocumentHeaderLayout {
    #[cfg_attr(not(test), allow(dead_code))]
    pub visible_indices: Vec<usize>,
    pub segment_rects: Vec<(usize, Rect)>,
    #[allow(dead_code)]
    pub height: f64,
}

pub fn header_height(has_breadcrumb: bool, has_right_dock: bool) -> f64 {
    header_height_for_document(has_breadcrumb, has_right_dock, false)
}

pub fn header_height_for_document(
    has_breadcrumb: bool,
    has_right_dock: bool,
    has_active_document: bool,
) -> f64 {
    if has_active_document || has_breadcrumb || has_right_dock {
        DOCUMENT_HEADER_H
    } else {
        0.0
    }
}

pub fn layout_header(
    available_width: f64,
    label_widths: &[f64],
    right_button_width: f64,
) -> DocumentHeaderLayout {
    let has_right_dock = right_button_width > 0.0;
    let height = header_height(!label_widths.is_empty(), has_right_dock);
    if available_width <= 0.0 || label_widths.is_empty() {
        return DocumentHeaderLayout {
            visible_indices: Vec::new(),
            segment_rects: Vec::new(),
            height,
        };
    }

    let content_width = (available_width
        - if has_right_dock {
            right_button_width
        } else {
            0.0
        }
        - HEADER_PAD_X)
        .max(0.0);
    let current = label_widths.len() - 1;
    let mut visible_indices = vec![current];
    let segment_width = |index: usize| label_widths[index].max(0.0) + 2.0 * SEGMENT_PAD_X;
    let mut used = segment_width(current);
    for index in (0..current).rev() {
        let next = SEPARATOR_SLOT_W + segment_width(index);
        if used + next > content_width {
            break;
        }
        visible_indices.push(index);
        used += next;
    }
    visible_indices.reverse();

    let content_end = content_width;
    let mut x = 0.0;
    let mut segment_rects = Vec::with_capacity(visible_indices.len());
    for (position, &index) in visible_indices.iter().enumerate() {
        let width = segment_width(index).min((content_end - x).max(0.0));
        segment_rects.push((
            index,
            Rect {
                pos: dvec2(HEADER_PAD_X + x, 0.0),
                size: dvec2(width, DOCUMENT_HEADER_H),
            },
        ));
        x += segment_width(index);
        if position + 1 < visible_indices.len() {
            x += SEPARATOR_SLOT_W;
        }
    }

    DocumentHeaderLayout {
        visible_indices,
        segment_rects,
        height,
    }
}

fn separator_rect(left_segment: Rect) -> Rect {
    Rect {
        pos: dvec2(
            left_segment.pos.x + left_segment.size.x + (SEPARATOR_SLOT_W - SEPARATOR_SIZE) * 0.5,
            left_segment.pos.y + (DOCUMENT_HEADER_H - SEPARATOR_SIZE) * 0.5,
        ),
        size: dvec2(SEPARATOR_SIZE, SEPARATOR_SIZE),
    }
}

/// `text_size` is the MEASURED line-box height, not `text_style.font_size`:
/// font sizes are points, so centring off them left the label off-axis against
/// the geometrically centred chevrons (see `separator_rect`).
fn centered_text_y(row: Rect, text_size: f64) -> f64 {
    (row.pos.y + (row.size.y - text_size) * 0.5 + TEXT_DY).round()
}

fn content_clip_rect(origin: DVec2, available_width: f64, right_button_width: f64) -> Rect {
    let reserved = if right_button_width > 0.0 {
        right_button_width
    } else {
        0.0
    };
    Rect {
        pos: origin,
        size: dvec2((available_width - reserved).max(0.0), DOCUMENT_HEADER_H),
    }
}

#[derive(Default)]
struct DocumentHeaderState {
    segments: Vec<BreadcrumbSegment>,
    right_dock: Option<Icon>,
    /// The destination action supplied by the active document, when present.
    view_toggle: Option<HeaderViewAction>,
    segment_rects: Vec<(usize, Rect)>,
    /// Keeps the header band mounted for an active document even before it has
    /// breadcrumbs, so the body does not reflow when they appear. The history
    /// controls themselves live on `tab_row` (see `App::sync_history_controls`).
    document_active: bool,
}

impl DocumentHeaderState {
    #[cfg(test)]
    fn for_test(
        segments: Vec<BreadcrumbSegment>,
        right_dock: Option<Icon>,
        segment_rects: Vec<(usize, Rect)>,
    ) -> Self {
        Self {
            segments,
            right_dock,
            view_toggle: None,
            segment_rects,
            document_active: false,
        }
    }

    fn replace_segments(&mut self, segments: Vec<BreadcrumbSegment>) -> bool {
        if self.segments == segments {
            return false;
        }
        self.segments = segments;
        self.segment_rects.clear();
        true
    }

    fn replace_right_dock(&mut self, right_dock: Option<Icon>) -> bool {
        if self.right_dock == right_dock {
            return false;
        }
        self.right_dock = right_dock;
        self.segment_rects.clear();
        true
    }

    fn replace_view_toggle(&mut self, view_toggle: Option<HeaderViewAction>) -> bool {
        if self.view_toggle == view_toggle {
            return false;
        }
        self.view_toggle = view_toggle;
        self.segment_rects.clear();
        true
    }

    /// Width the trailing buttons reserve on the right of the header.
    fn trailing_buttons_width(&self) -> f64 {
        let mut width = 0.0;
        if self.view_toggle.is_some() {
            width += DOCUMENT_HEADER_H;
        }
        if self.right_dock.is_some() {
            width += DOCUMENT_HEADER_H;
        }
        width
    }

    fn visible_height(&self) -> f64 {
        header_height_for_document(
            !self.segments.is_empty(),
            self.right_dock.is_some() || self.view_toggle.is_some(),
            self.document_active,
        )
    }

    fn action_at(&self, position: DVec2) -> Option<DocumentHeaderAction> {
        self.segment_rects
            .iter()
            .rev()
            .find(|(_, rect)| rect.size.x > 0.0 && rect.size.y > 0.0 && rect.contains(position))
            .and_then(|(index, _)| self.segments.get(*index))
            .map(|segment| DocumentHeaderAction::RevealInTree(segment.target.clone()))
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct DocumentHeader {
    #[deref]
    view: View,
    #[live]
    draw_ancestor: DrawText,
    #[live]
    draw_current: DrawText,
    #[live]
    draw_border: DrawColor,
    #[live]
    draw_separator: DrawColor,
    #[rust]
    state: DocumentHeaderState,
    #[rust]
    draw_rect: Rect,
    #[rust]
    right_button_uid: Option<WidgetUid>,
}

impl Widget for DocumentHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                let event_rect = self.view.area().rect(cx);
                let hit_offset = event_rect.pos - self.draw_rect.pos;
                if let Some(action) = self.state.action_at(fe.abs - hit_offset) {
                    cx.widget_action(self.widget_uid(), action);
                }
            }
            // `HoverOver` as well as `HoverIn`: only some of the header is
            // actionable (the breadcrumb segments), so the cursor has to track
            // the pointer *within* the widget, not just its entry.
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let event_rect = self.view.area().rect(cx);
                let hit_offset = event_rect.pos - self.draw_rect.pos;
                if self.state.action_at(fe.abs - hit_offset).is_some() {
                    cursor::hover_in(cx, MouseCursor::Hand);
                } else {
                    cursor::hover_out(cx);
                }
            }
            Hit::FingerHoverOut(_) => cursor::hover_out(cx),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        self.draw_rect = self.view.area().rect(cx);

        let label_widths = self
            .state
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let draw = if index + 1 == self.state.segments.len() {
                    &self.draw_current
                } else {
                    &self.draw_ancestor
                };
                let size = draw
                    .layout(cx, 0.0, 0.0, None, false, Align::default(), &segment.title)
                    .size_in_lpxs;
                (size.width as f64, size.height as f64)
            })
            .collect::<Vec<_>>();
        let label_heights = label_widths.iter().map(|(_, h)| *h).collect::<Vec<_>>();
        let label_widths = label_widths.iter().map(|(w, _)| *w).collect::<Vec<_>>();
        let right_button_width = self.state.trailing_buttons_width();
        let layout = layout_header(self.draw_rect.size.x, &label_widths, right_button_width);
        self.state.segment_rects = layout
            .segment_rects
            .iter()
            .map(|(index, rect)| {
                (
                    *index,
                    Rect {
                        pos: rect.pos + self.draw_rect.pos,
                        size: rect.size,
                    },
                )
            })
            .collect();

        if self.draw_rect.size.y > 0.0 {
            self.draw_border.draw_abs(
                cx,
                Rect {
                    pos: dvec2(
                        self.draw_rect.pos.x,
                        self.draw_rect.pos.y + DOCUMENT_HEADER_H - 1.0,
                    ),
                    size: dvec2(self.draw_rect.size.x, 1.0),
                },
            );
        }

        cx.push_clip_rect(content_clip_rect(
            self.draw_rect.pos,
            self.draw_rect.size.x,
            right_button_width,
        ));
        for (position, (index, rect)) in self.state.segment_rects.iter().enumerate() {
            let segment = &self.state.segments[*index];
            let draw = if *index + 1 == self.state.segments.len() {
                &mut self.draw_current
            } else {
                &mut self.draw_ancestor
            };
            let y = centered_text_y(*rect, label_heights[*index]);
            draw.draw_abs(cx, dvec2(rect.pos.x + SEGMENT_PAD_X, y), &segment.title);
            if position + 1 < self.state.segment_rects.len() {
                self.draw_separator.draw_abs(cx, separator_rect(*rect));
            }
        }
        cx.pop_clip_rect();

        step
    }
}

impl DocumentHeader {
    fn sync_content_layout(&mut self, cx: &mut Cx) {
        let height = self.state.visible_height();
        self.view.walk.height = Size::Fixed(height);
        cx.redraw_all();
    }

    pub fn set_segments(&mut self, cx: &mut Cx, segments: Vec<BreadcrumbSegment>) {
        if self.state.replace_segments(segments) {
            self.sync_content_layout(cx);
        }
    }

    pub fn set_right_dock(&mut self, cx: &mut Cx, icon: Option<Icon>) {
        if !self.state.replace_right_dock(icon) {
            return;
        }

        let button = self.view.widget(cx, ids!(right_button));
        self.right_button_uid = Some(button.widget_uid());
        button.set_visible(cx, icon.is_some());
        if let Some(icon) = icon {
            button.as_icon_button().set_icon(cx, icon);
        }
        self.sync_content_layout(cx);
    }

    pub fn set_view_toggle(&mut self, cx: &mut Cx, action: Option<HeaderViewAction>) {
        if !self.state.replace_view_toggle(action) {
            return;
        }

        let button = self.view.widget(cx, ids!(view_button));
        button.set_visible(cx, action.is_some());
        if let Some(action) = action {
            button.as_icon_button().set_icon(cx, action.icon);
            button
                .as_icon_button()
                .set_tooltip(cx, Some(action.tooltip));
        } else {
            button.as_icon_button().set_tooltip(cx, None);
        }
        self.sync_content_layout(cx);
    }

    /// The active document's view action button, for click detection. Chrome
    /// projection owns its icon and tooltip.
    pub fn view_action_button(&self, cx: &mut Cx) -> WidgetRef {
        self.view.widget(cx, ids!(view_button))
    }

    pub fn set_right_dock_icon(&mut self, cx: &mut Cx, icon: Icon) {
        let Some(current) = self.state.right_dock.as_mut() else {
            return;
        };
        if *current == icon {
            return;
        }
        *current = icon;
        self.view
            .widget(cx, ids!(right_button))
            .as_icon_button()
            .set_icon(cx, icon);
    }

    pub fn set_right_dock_active(&mut self, cx: &mut Cx, active: bool) {
        self.view
            .widget(cx, ids!(right_button))
            .as_icon_button()
            .set_active(cx, active);
    }

    pub fn set_document_active(&mut self, cx: &mut Cx, active: bool) {
        if self.state.document_active == active {
            return;
        }
        self.state.document_active = active;
        self.sync_content_layout(cx);
    }

    pub fn visible_height(&self) -> f64 {
        self.state.visible_height()
    }

    #[cfg(test)]
    pub(crate) fn test_segments(&self) -> &[BreadcrumbSegment] {
        &self.state.segments
    }

    #[cfg(test)]
    pub(crate) fn test_right_dock(&self) -> Option<Icon> {
        self.state.right_dock
    }

    #[cfg(test)]
    pub(crate) fn test_view_toggle(&self) -> Option<HeaderViewAction> {
        self.state.view_toggle
    }

    #[cfg(test)]
    pub(crate) fn test_mount_view_action_button(&mut self, button: WidgetRef) {
        self.view.children.push((live_id!(view_button), button));
    }

    #[cfg(test)]
    pub(crate) fn test_right_dock_active(&self, cx: &mut Cx) -> Option<bool> {
        let button = self.view.widget(cx, ids!(right_button)).as_icon_button();
        let rect = button.rect(cx);
        if rect.size.x <= 0.0 || rect.size.y <= 0.0 {
            return None;
        }

        // `IconButton::set_active` redraws only when its real state changes.
        // Probe `true`, observe that idempotence, then restore `false` when the
        // probe changed it. Preserve the draw event that was pending before this
        // test-only query so observation itself has no externally visible effect.
        let pending = std::mem::take(&mut cx.new_draw_event);
        button.set_active(cx, true);
        let was_active = !cx.new_draw_event.will_redraw();
        if !was_active {
            button.set_active(cx, false);
        }
        cx.new_draw_event = pending;
        Some(was_active)
    }

    pub fn action(&self, actions: &Actions) -> Option<DocumentHeaderAction> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let Some(action) = item.action.downcast_ref::<DocumentHeaderAction>() {
                return Some(action.clone());
            }
        }

        let clicked = |uid: Option<WidgetUid>| {
            uid.and_then(|uid| actions.find_widget_action(uid))
                .is_some_and(|item| {
                    matches!(
                        item.action.downcast_ref::<IconButtonAction>(),
                        Some(IconButtonAction::Clicked)
                    )
                })
        };
        if clicked(self.right_button_uid) {
            Some(DocumentHeaderAction::ToggleRightDock)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_view::HeaderViewAction;
    use crate::icons::Icon;
    use crate::navigation::{BreadcrumbSegment, NavigationTarget};

    fn segment(title: &str, concept_id: &str) -> BreadcrumbSegment {
        BreadcrumbSegment {
            title: title.into(),
            target: NavigationTarget::Document {
                concept_id: concept_id.into(),
                surface: None,
                fragment: None,
            },
        }
    }

    #[test]
    fn header_height_tracks_its_two_content_sources() {
        assert_eq!(header_height(false, false), 0.0);
        assert_eq!(header_height(true, false), DOCUMENT_HEADER_H);
        assert_eq!(header_height(false, true), DOCUMENT_HEADER_H);
        assert_eq!(header_height(true, true), DOCUMENT_HEADER_H);
    }

    #[test]
    fn layout_matches_reference_inset_padding_and_separator_spacing() {
        let layout = layout_header(260.0, &[20.0, 30.0], 0.0);
        assert_eq!(layout.segment_rects[0].1.pos.x, HEADER_PAD_X);
        assert_eq!(layout.segment_rects[0].1.size.x, 32.0);
        assert_eq!(layout.segment_rects[1].1.pos.x, 52.0);
        assert_eq!(layout.segment_rects[1].1.size.x, 42.0);

        let separator = separator_rect(layout.segment_rects[0].1);
        assert_eq!(separator.pos.x, 41.0);
        assert_eq!(separator.size, dvec2(10.0, 10.0));
        assert_eq!(layout.segment_rects[0].1.pos.x + SEGMENT_PAD_X, 14.0);
    }

    #[test]
    fn text_is_pixel_snapped_and_optically_centered() {
        let row = Rect {
            pos: dvec2(10.25, 5.25),
            size: dvec2(80.0, DOCUMENT_HEADER_H),
        };
        assert_eq!(centered_text_y(row, 10.0), 16.0);
        assert_eq!(centered_text_y(row, 11.0), 16.0);
    }

    #[test]
    fn padded_hit_rects_keep_original_navigation_targets() {
        let segments = vec![segment("Root", "root"), segment("Current", "current")];
        let layout = layout_header(180.0, &[24.0, 42.0], 0.0);
        let state =
            DocumentHeaderState::for_test(segments.clone(), None, layout.segment_rects.clone());

        for (index, rect) in &layout.segment_rects {
            assert!(rect.size.x >= 12.0);
            assert_eq!(
                state.action_at(rect.pos + rect.size * 0.5),
                Some(DocumentHeaderAction::RevealInTree(
                    segments[*index].target.clone()
                ))
            );
        }
    }

    #[test]
    fn constrained_positive_content_keeps_a_positive_current_hit_rect() {
        let layout = layout_header(115.0, &[44.0, 58.0], 30.0);
        assert_eq!(layout.visible_indices, vec![1]);
        assert!(layout.segment_rects[0].1.size.x > 0.0);
    }

    #[test]
    fn active_document_keeps_the_header_mounted_without_breadcrumbs() {
        assert_eq!(
            header_height_for_document(false, false, true),
            DOCUMENT_HEADER_H
        );
        assert_eq!(header_height_for_document(false, false, false), 0.0);
    }

    #[test]
    fn breadcrumbs_start_at_the_header_padding() {
        let layout = layout_header(180.0, &[40.0, 50.0], 0.0);
        assert_eq!(
            layout.segment_rects.first().map(|(_, rect)| rect.pos.x),
            Some(HEADER_PAD_X)
        );
    }

    #[test]
    fn narrow_elision_preserves_the_current_segment() {
        let layout = layout_header(90.0, &[44.0, 52.0, 58.0], 0.0);
        assert_eq!(layout.visible_indices.last(), Some(&2));
        assert!(!layout.visible_indices.contains(&0));
    }

    #[test]
    fn positive_width_keeps_current_even_when_button_uses_the_content_width() {
        let layout = layout_header(30.0, &[44.0, 58.0], 30.0);
        assert_eq!(layout.visible_indices, vec![1]);
        assert_eq!(layout.segment_rects[0].1.size.x, 0.0);
    }

    #[test]
    fn every_positive_width_keeps_the_current_segment_in_layout() {
        for width in [f64::MIN_POSITIVE, 0.01, 1.0, 29.9, 30.0, 31.0, 240.0] {
            let layout = layout_header(width, &[44.0, 52.0, 58.0], 30.0);
            assert_eq!(
                layout.visible_indices.last(),
                Some(&2),
                "current segment missing at width {width}"
            );
            assert_eq!(
                layout.segment_rects.last().map(|(index, _)| *index),
                Some(2),
                "current hit geometry missing at width {width}"
            );
        }
    }

    #[test]
    fn right_button_reservation_elides_only_the_oldest_ancestor() {
        let without_button = layout_header(170.0, &[30.0, 30.0, 30.0], 0.0);
        assert_eq!(without_button.visible_indices, vec![0, 1, 2]);

        let with_button = layout_header(170.0, &[30.0, 30.0, 30.0], 30.0);
        assert_eq!(with_button.visible_indices, vec![1, 2]);
        assert_eq!(with_button.segment_rects[0].1.pos.x, HEADER_PAD_X);
        assert_eq!(with_button.segment_rects[1].1.pos.x, 62.0);
    }

    #[test]
    fn replacing_the_view_destination_preserves_it_in_one_button_slot() {
        let mut state = DocumentHeaderState::for_test(Vec::new(), None, Vec::new());

        assert!(state.replace_view_toggle(Some(HeaderViewAction {
            icon: Icon::Code,
            tooltip: "View source",
        })));
        assert_eq!(state.trailing_buttons_width(), DOCUMENT_HEADER_H);

        let destination = HeaderViewAction {
            icon: Icon::Eye,
            tooltip: "View rendered",
        };
        assert!(state.replace_view_toggle(Some(destination)));
        assert_eq!(state.view_toggle, Some(destination));
        assert_eq!(state.trailing_buttons_width(), DOCUMENT_HEADER_H);
    }

    #[test]
    fn content_clip_stops_at_the_right_button_edge() {
        assert_eq!(
            content_clip_rect(dvec2(10.0, 5.0), 120.0, 30.0),
            Rect {
                pos: dvec2(10.0, 5.0),
                size: dvec2(90.0, DOCUMENT_HEADER_H),
            }
        );
        assert_eq!(
            content_clip_rect(dvec2(10.0, 5.0), 120.0, 0.0),
            Rect {
                pos: dvec2(10.0, 5.0),
                size: dvec2(120.0, DOCUMENT_HEADER_H),
            }
        );
    }

    #[test]
    fn hit_rects_retain_original_segment_indices() {
        let layout = layout_header(330.0, &[40.0, 50.0, 60.0], 30.0);
        assert_eq!(
            layout
                .segment_rects
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn state_transitions_clear_stale_segments_and_right_dock_icon() {
        let root = segment("Root", "root");
        let current = segment("Current", "current");

        let breadcrumb_only =
            DocumentHeaderState::for_test(vec![current.clone()], None, Vec::new());
        assert_eq!(breadcrumb_only.visible_height(), DOCUMENT_HEADER_H);
        assert_eq!(breadcrumb_only.segments, vec![current.clone()]);
        assert_eq!(breadcrumb_only.right_dock, None);

        let button_only =
            DocumentHeaderState::for_test(Vec::new(), Some(Icon::Package), Vec::new());
        assert_eq!(button_only.visible_height(), DOCUMENT_HEADER_H);
        assert!(button_only.segments.is_empty());
        assert_eq!(button_only.right_dock, Some(Icon::Package));

        let mut combined =
            DocumentHeaderState::for_test(vec![root, current], Some(Icon::Package), Vec::new());
        assert_eq!(combined.visible_height(), DOCUMENT_HEADER_H);
        assert!(combined.replace_segments(Vec::new()));
        assert!(combined.segments.is_empty());
        assert_eq!(combined.right_dock, Some(Icon::Package));
        assert!(combined.replace_right_dock(None));
        assert_eq!(combined.visible_height(), 0.0);
        assert!(combined.segments.is_empty());
        assert_eq!(combined.right_dock, None);

        let empty = DocumentHeaderState::for_test(Vec::new(), None, Vec::new());
        assert_eq!(empty.visible_height(), 0.0);
    }

    #[test]
    fn clicking_current_segment_emits_its_document_target() {
        let expected = NavigationTarget::Document {
            concept_id: "current".into(),
            surface: None,
            fragment: None,
        };
        let state = DocumentHeaderState::for_test(
            vec![segment("Root", "root"), segment("Current", "current")],
            None,
            vec![
                (
                    0,
                    Rect {
                        pos: dvec2(0.0, 0.0),
                        size: dvec2(40.0, DOCUMENT_HEADER_H),
                    },
                ),
                (
                    1,
                    Rect {
                        pos: dvec2(54.0, 0.0),
                        size: dvec2(60.0, DOCUMENT_HEADER_H),
                    },
                ),
            ],
        );

        assert_eq!(
            state.action_at(dvec2(80.0, 15.0)),
            Some(DocumentHeaderAction::RevealInTree(expected))
        );
    }

    #[test]
    fn every_visible_segment_hit_rect_maps_to_its_original_target() {
        let segments = vec![
            BreadcrumbSegment {
                title: "Root".into(),
                target: NavigationTarget::Directory {
                    address: "/".into(),
                },
            },
            BreadcrumbSegment {
                title: "Sales".into(),
                target: NavigationTarget::Directory {
                    address: "/sales".into(),
                },
            },
            segment("Archive", "sales/archive"),
            segment("Order", "sales/archive/order"),
        ];
        let layout = layout_header(260.0, &[36.0, 42.0, 48.0, 50.0], 30.0);
        let state = DocumentHeaderState::for_test(
            segments.clone(),
            Some(Icon::PanelRight),
            layout.segment_rects.clone(),
        );

        for (index, rect) in &layout.segment_rects {
            assert!(rect.size.x > 0.0);
            assert_eq!(
                state.action_at(rect.pos + rect.size * 0.5),
                Some(DocumentHeaderAction::RevealInTree(
                    segments[*index].target.clone()
                )),
                "hit for visible segment {index}"
            );
        }
    }

    #[test]
    fn unchanged_content_keeps_existing_hit_geometry_valid() {
        let current = segment("Current", "current");
        let rects = vec![(
            0,
            Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(60.0, DOCUMENT_HEADER_H),
            },
        )];
        let mut state = DocumentHeaderState::for_test(
            vec![current.clone()],
            Some(Icon::Package),
            rects.clone(),
        );

        assert!(!state.replace_segments(vec![current]));
        assert!(!state.replace_right_dock(Some(Icon::Package)));
        assert_eq!(state.segment_rects, rects);
    }

    #[test]
    fn content_transition_requests_root_relayout() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut header = cx.with_vm(DocumentHeader::script_new_with_default);
        cx.new_draw_event = DrawEvent::default();

        header.set_segments(&mut cx, vec![segment("Current", "current")]);

        assert_eq!(header.visible_height(), DOCUMENT_HEADER_H);
        assert!(
            cx.new_draw_event.redraw_all,
            "changing header height must invalidate the parent flow"
        );

        header.set_segments(&mut cx, Vec::new());
        assert_eq!(header.visible_height(), 0.0);
    }

    #[test]
    fn right_dock_glyph_tracks_the_next_action() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut header = cx.with_vm(DocumentHeader::script_new_with_default);

        header.set_right_dock(&mut cx, Some(Icon::PanelRight));
        header.set_right_dock_icon(&mut cx, Icon::PanelRightOpen);
        assert_eq!(header.test_right_dock(), Some(Icon::PanelRightOpen));

        header.set_right_dock_icon(&mut cx, Icon::PanelRightClose);
        assert_eq!(header.test_right_dock(), Some(Icon::PanelRightClose));

        header.set_right_dock(&mut cx, None);
        header.set_right_dock_icon(&mut cx, Icon::PanelRightOpen);
        assert_eq!(header.test_right_dock(), None);
    }
}
