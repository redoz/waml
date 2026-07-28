// App/layout mounting lands in follow-up tasks; keep the shared widget lint-clean meanwhile.
#![allow(dead_code)]

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

    mod.widgets.DocumentHeader = set_type_default() do mod.widgets.DocumentHeaderBase{
        width: Fill
        height: 0.0
        flow: Right
        align: Align{y: 0.5}
        clip_x: true

        draw_ancestor +: {
            color: atlas.text_dim
            text_style: fonts.text_menu
        }
        draw_current +: {
            color: atlas.text
            text_style: fonts.text_label
        }
        draw_chevron +: {
            color: atlas.text_dim
            text_style: fonts.text_menu
        }

        breadcrumb_slot := View {
            width: Fill
            height: Fill
        }
        right_button := IconButton {
            visible: false
            width: 30.0
            height: 30.0
        }
    }
}

pub const DOCUMENT_HEADER_H: f64 = 30.0;
const CHEVRON_W: f64 = 14.0;

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentHeaderAction {
    Navigate(NavigationTarget),
    ToggleRightDock,
}

pub struct DocumentHeaderLayout {
    pub visible_indices: Vec<usize>,
    pub segment_rects: Vec<(usize, Rect)>,
    pub height: f64,
}

pub fn header_height(has_breadcrumb: bool, has_right_dock: bool) -> f64 {
    if has_breadcrumb || has_right_dock {
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
        })
    .max(0.0);
    let current = label_widths.len() - 1;
    let mut visible_indices = vec![current];
    let mut used = label_widths[current].max(0.0);
    for index in (0..current).rev() {
        let next = CHEVRON_W + label_widths[index].max(0.0);
        if used + next > content_width {
            break;
        }
        visible_indices.push(index);
        used += next;
    }
    visible_indices.reverse();

    let mut x = 0.0;
    let mut segment_rects = Vec::with_capacity(visible_indices.len());
    for (position, &index) in visible_indices.iter().enumerate() {
        let width = label_widths[index]
            .max(0.0)
            .min((content_width - x).max(0.0));
        segment_rects.push((
            index,
            Rect {
                pos: dvec2(x, 0.0),
                size: dvec2(width, DOCUMENT_HEADER_H),
            },
        ));
        x += label_widths[index].max(0.0);
        if position + 1 < visible_indices.len() {
            x += CHEVRON_W;
        }
    }

    DocumentHeaderLayout {
        visible_indices,
        segment_rects,
        height,
    }
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
    segment_rects: Vec<(usize, Rect)>,
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
            segment_rects,
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

    fn visible_height(&self) -> f64 {
        header_height(!self.segments.is_empty(), self.right_dock.is_some())
    }

    fn action_at(&self, position: DVec2) -> Option<DocumentHeaderAction> {
        self.segment_rects
            .iter()
            .rev()
            .find(|(_, rect)| rect.size.x > 0.0 && rect.size.y > 0.0 && rect.contains(position))
            .and_then(|(index, _)| self.segments.get(*index))
            .map(|segment| DocumentHeaderAction::Navigate(segment.target.clone()))
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
    draw_chevron: DrawText,
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
            Hit::FingerHoverIn(fe) => {
                let event_rect = self.view.area().rect(cx);
                let hit_offset = event_rect.pos - self.draw_rect.pos;
                if self.state.action_at(fe.abs - hit_offset).is_some() {
                    cx.set_cursor(MouseCursor::Hand);
                }
            }
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
                draw.layout(cx, 0.0, 0.0, None, false, Align::default(), &segment.title)
                    .size_in_lpxs
                    .width as f64
            })
            .collect::<Vec<_>>();
        let right_button_width = if self.state.right_dock.is_some() {
            DOCUMENT_HEADER_H
        } else {
            0.0
        };
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
            let y = rect.pos.y + (DOCUMENT_HEADER_H - draw.text_style.font_size as f64) * 0.5;
            draw.draw_abs(cx, dvec2(rect.pos.x, y), &segment.title);
            if position + 1 < self.state.segment_rects.len() {
                let chevron_y = rect.pos.y
                    + (DOCUMENT_HEADER_H - self.draw_chevron.text_style.font_size as f64) * 0.5;
                self.draw_chevron
                    .draw_abs(cx, dvec2(rect.pos.x + rect.size.x, chevron_y), ">");
            }
        }
        cx.pop_clip_rect();

        step
    }
}

impl DocumentHeader {
    fn sync_content_layout(&mut self, cx: &mut Cx) {
        self.view.walk.height = Size::Fixed(self.state.visible_height());
        self.view.redraw(cx);
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

    pub fn set_right_dock_active(&mut self, cx: &mut Cx, active: bool) {
        self.view
            .widget(cx, ids!(right_button))
            .as_icon_button()
            .set_active(cx, active);
    }

    pub fn visible_height(&self) -> f64 {
        self.state.visible_height()
    }

    pub fn action(&self, actions: &Actions) -> Option<DocumentHeaderAction> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let Some(action) = item.action.downcast_ref::<DocumentHeaderAction>() {
                return Some(action.clone());
            }
        }

        let button_uid = self.right_button_uid?;
        let item = actions.find_widget_action(button_uid)?;
        matches!(
            item.action.downcast_ref::<IconButtonAction>(),
            Some(IconButtonAction::Clicked)
        )
        .then_some(DocumentHeaderAction::ToggleRightDock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Icon;
    use crate::navigation::{BreadcrumbSegment, NavigationTarget};

    fn segment(title: &str, concept_id: &str) -> BreadcrumbSegment {
        BreadcrumbSegment {
            title: title.into(),
            target: NavigationTarget::Document {
                concept_id: concept_id.into(),
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
    fn right_button_reservation_elides_only_the_oldest_ancestor() {
        let without_button = layout_header(120.0, &[30.0, 30.0, 30.0], 0.0);
        assert_eq!(without_button.visible_indices, vec![0, 1, 2]);

        let with_button = layout_header(120.0, &[30.0, 30.0, 30.0], 30.0);
        assert_eq!(with_button.visible_indices, vec![1, 2]);
        assert_eq!(with_button.segment_rects[0].1.pos.x, 0.0);
        assert_eq!(with_button.segment_rects[1].1.pos.x, 44.0);
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
        let layout = layout_header(300.0, &[40.0, 50.0, 60.0], 30.0);
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
            Some(DocumentHeaderAction::Navigate(expected))
        );
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
}
