use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.ToggleControlBase = #(ToggleControl::register_widget(vm))
    mod.widgets.ToggleControl = set_type_default() do mod.widgets.ToggleControlBase{
        width: 36.0
        height: 22.0
        draw_track_off +: { color: atlas.surface_border }
        draw_track_on +: { color: atlas.accent }
        draw_knob +: { color: atlas.field_bg }
        draw_hover +: { color: atlas.accent_soft }
        draw_focus +: { color: atlas.accent }
        draw_disabled +: { color: atlas.text_dim }
    }

    mod.widgets.SegmentedControlBase = #(SegmentedControl::register_widget(vm))
    mod.widgets.SegmentedControl = set_type_default() do mod.widgets.SegmentedControlBase{
        width: Fill
        height: 30.0
        draw_bg +: { color: atlas.field_bg }
        draw_selected +: { color: atlas.selection }
        draw_hover +: { color: atlas.accent_soft }
        draw_border +: { color: atlas.surface_border }
        draw_focus +: { color: atlas.accent }
        draw_label +: {
            color: atlas.text
            text_style: fonts.text_label
        }
        draw_label_selected +: {
            color: atlas.accent
            text_style: fonts.text_label
        }
        draw_label_disabled +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentedState<T> {
    items: Vec<T>,
    selected: usize,
}

#[cfg(test)]
impl<T: Clone + PartialEq> SegmentedState<T> {
    pub fn new(items: Vec<T>, selected: T) -> Self {
        assert!(
            !items.is_empty(),
            "segmented controls require at least one item"
        );
        let selected = items.iter().position(|item| item == &selected).unwrap_or(0);
        Self { items, selected }
    }

    pub fn selected(&self) -> T {
        self.items[self.selected].clone()
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn select_previous(&mut self) {
        self.selected = (self.selected + self.items.len() - 1) % self.items.len();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SegmentItem {
    pub id: LiveId,
    pub label: String,
}

impl SegmentItem {
    pub fn new(id: LiveId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug)]
enum ToggleControlAction {
    Changed(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ControlAccess {
    register_nav_stop: bool,
    retain_focus: bool,
}

fn control_access(enabled: bool, focused: bool) -> ControlAccess {
    ControlAccess {
        register_nav_stop: enabled,
        retain_focus: enabled && focused,
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct ToggleControl {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_track_off: DrawColor,
    #[redraw]
    #[live]
    draw_track_on: DrawColor,
    #[redraw]
    #[live]
    draw_knob: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    #[redraw]
    #[live]
    draw_focus: DrawColor,
    #[redraw]
    #[live]
    draw_disabled: DrawColor,
    #[rust]
    value: bool,
    #[rust(true)]
    enabled: bool,
    #[rust]
    hovered: bool,
    #[rust]
    focused: bool,
}

impl ToggleControl {
    pub fn set_value(&mut self, cx: &mut Cx, value: bool) {
        if self.value != value {
            self.value = value;
            self.draw_track_off.redraw(cx);
        }
    }

    pub fn set_enabled(&mut self, cx: &mut Cx, enabled: bool) {
        if self.enabled != enabled {
            let access = control_access(enabled, self.focused);
            if !access.retain_focus
                && (self.focused || cx.has_key_focus(self.draw_track_off.area()))
            {
                cx.set_key_focus(Area::Empty);
            }
            self.focused = access.retain_focus;
            self.enabled = enabled;
            self.draw_track_off.redraw(cx);
        }
    }

    pub fn changed(&self, actions: &Actions) -> Option<bool> {
        let action = actions.find_widget_action(self.widget_uid())?;
        match action.action.downcast_ref::<ToggleControlAction>()? {
            ToggleControlAction::Changed(value) => Some(*value),
        }
    }

    fn toggle(&mut self, cx: &mut Cx) {
        if !self.enabled {
            return;
        }
        self.value = !self.value;
        self.draw_track_off.redraw(cx);
        cx.widget_action(self.widget_uid(), ToggleControlAction::Changed(self.value));
    }
}

impl Widget for ToggleControl {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.draw_track_off.area()) {
            Hit::KeyFocus(_) if self.enabled => {
                self.focused = true;
                self.draw_track_off.redraw(cx);
            }
            Hit::KeyFocus(_) => {
                self.focused = false;
                cx.set_key_focus(Area::Empty);
                self.draw_track_off.redraw(cx);
            }
            Hit::KeyFocusLost(_) => {
                self.focused = false;
                self.draw_track_off.redraw(cx);
            }
            Hit::FingerHoverIn(_) => {
                if self.enabled {
                    cx.set_cursor(MouseCursor::Hand);
                }
                self.hovered = true;
                self.draw_track_off.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = false;
                self.draw_track_off.redraw(cx);
            }
            Hit::FingerDown(fe) if self.enabled && fe.is_primary_hit() => {
                cx.set_key_focus(self.draw_track_off.area());
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => self.toggle(cx),
            Hit::KeyDown(ke) if matches!(ke.key_code, KeyCode::Space | KeyCode::ReturnKey) => {
                self.toggle(cx)
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if self.hovered && self.enabled {
            self.draw_hover.draw_abs(cx, rect);
        }
        let track = Rect {
            pos: dvec2(rect.pos.x + 2.0, rect.pos.y + 3.0),
            size: dvec2((rect.size.x - 4.0).max(0.0), (rect.size.y - 6.0).max(0.0)),
        };
        // The off-track is also the stable hit area, so draw it for every
        // state (including an initially-on value) before layering state ink.
        self.draw_track_off.draw_abs(cx, track);
        if !self.enabled {
            self.draw_disabled.draw_abs(cx, track);
        } else if self.value {
            self.draw_track_on.draw_abs(cx, track);
        }
        let knob_side = (track.size.y - 4.0).max(0.0);
        let knob_x = if self.value {
            track.pos.x + track.size.x - knob_side - 2.0
        } else {
            track.pos.x + 2.0
        };
        self.draw_knob.draw_abs(
            cx,
            Rect {
                pos: dvec2(knob_x, track.pos.y + 2.0),
                size: dvec2(knob_side, knob_side),
            },
        );
        if self.focused {
            draw_outline(cx, &mut self.draw_focus, rect, 1.5);
        }
        if control_access(self.enabled, self.focused).register_nav_stop {
            cx.add_nav_stop(
                self.draw_track_off.area(),
                NavRole::TextInput,
                Inset::default(),
            );
        }
        DrawStep::done()
    }
}

#[derive(Clone, Debug)]
enum SegmentedControlAction {
    Changed(LiveId),
}

#[derive(Script, ScriptHook, Widget)]
pub struct SegmentedControl {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[redraw]
    #[live]
    draw_selected: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    #[redraw]
    #[live]
    draw_border: DrawColor,
    #[redraw]
    #[live]
    draw_focus: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[redraw]
    #[live]
    draw_label_selected: DrawText,
    #[redraw]
    #[live]
    draw_label_disabled: DrawText,
    #[rust]
    items: Vec<SegmentItem>,
    #[rust]
    selected: LiveId,
    #[rust]
    segment_rects: Vec<(LiveId, Rect)>,
    #[rust]
    hovered: Option<LiveId>,
    #[rust]
    focused: bool,
    #[rust(true)]
    enabled: bool,
}

impl SegmentedControl {
    pub fn set_items(&mut self, cx: &mut Cx, items: Vec<SegmentItem>) {
        if self.items != items {
            self.items = items;
            if !self.items.iter().any(|item| item.id == self.selected) {
                self.selected = self.items.first().map(|item| item.id).unwrap_or_default();
            }
            self.draw_bg.redraw(cx);
        }
    }

    pub fn set_selected(&mut self, cx: &mut Cx, id: LiveId) {
        if self.selected != id && self.items.iter().any(|item| item.id == id) {
            self.selected = id;
            self.draw_bg.redraw(cx);
        }
    }

    pub fn changed(&self, actions: &Actions) -> Option<LiveId> {
        let action = actions.find_widget_action(self.widget_uid())?;
        match action.action.downcast_ref::<SegmentedControlAction>()? {
            SegmentedControlAction::Changed(id) => Some(*id),
        }
    }

    fn select(&mut self, cx: &mut Cx, id: LiveId) {
        if !self.enabled || self.selected == id || !self.items.iter().any(|item| item.id == id) {
            return;
        }
        self.selected = id;
        self.draw_bg.redraw(cx);
        cx.widget_action(self.widget_uid(), SegmentedControlAction::Changed(id));
    }

    fn select_offset(&mut self, cx: &mut Cx, delta: isize) {
        if !self.enabled || self.items.is_empty() {
            return;
        }
        let current = self
            .items
            .iter()
            .position(|item| item.id == self.selected)
            .unwrap_or(0);
        let len = self.items.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        self.select(cx, self.items[next].id);
    }
}

impl Widget for SegmentedControl {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.draw_bg.area()) {
            Hit::KeyFocus(_) if self.enabled => {
                self.focused = true;
                self.draw_bg.redraw(cx);
            }
            Hit::KeyFocus(_) => {
                self.focused = false;
                cx.set_key_focus(Area::Empty);
                self.draw_bg.redraw(cx);
            }
            Hit::KeyFocusLost(_) => {
                self.focused = false;
                self.draw_bg.redraw(cx);
            }
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                self.hovered = self
                    .segment_rects
                    .iter()
                    .find(|(_, rect)| rect.contains(fe.abs))
                    .map(|(id, _)| *id);
                if self.enabled {
                    cx.set_cursor(MouseCursor::Hand);
                }
                self.draw_bg.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = None;
                self.draw_bg.redraw(cx);
            }
            Hit::FingerDown(fe) if self.enabled && fe.is_primary_hit() => {
                cx.set_key_focus(self.draw_bg.area());
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                if let Some(id) = self
                    .segment_rects
                    .iter()
                    .find(|(_, rect)| rect.contains(fe.abs))
                    .map(|(id, _)| *id)
                {
                    self.select(cx, id);
                }
            }
            Hit::KeyDown(ke) => match ke.key_code {
                KeyCode::ArrowLeft | KeyCode::ArrowUp => self.select_offset(cx, -1),
                KeyCode::ArrowRight | KeyCode::ArrowDown => self.select_offset(cx, 1),
                _ => {}
            },
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        self.segment_rects.clear();
        if self.items.is_empty() {
            return DrawStep::done();
        }

        let segment_width = rect.size.x / self.items.len() as f64;
        for (index, item) in self.items.iter().enumerate() {
            let left = rect.pos.x + segment_width * index as f64;
            let right = if index + 1 == self.items.len() {
                rect.pos.x + rect.size.x
            } else {
                left + segment_width
            };
            let segment = Rect {
                pos: dvec2(left, rect.pos.y),
                size: dvec2((right - left).max(0.0), rect.size.y),
            };
            self.segment_rects.push((item.id, segment));
            if self.enabled && self.hovered == Some(item.id) && self.selected != item.id {
                self.draw_hover.draw_abs(cx, segment);
            }
            if self.selected == item.id {
                self.draw_selected.draw_abs(cx, segment);
            }
            if index > 0 {
                self.draw_border.draw_abs(
                    cx,
                    Rect {
                        pos: segment.pos,
                        size: dvec2(1.0, segment.size.y),
                    },
                );
            }
            let text = if !self.enabled {
                &mut self.draw_label_disabled
            } else if self.selected == item.id {
                &mut self.draw_label_selected
            } else {
                &mut self.draw_label
            };
            text.draw_abs(
                cx,
                dvec2(
                    segment.pos.x + 9.0,
                    segment.pos.y + segment.size.y * 0.5 - 6.0,
                ),
                &item.label,
            );
        }
        draw_outline(cx, &mut self.draw_border, rect, 1.0);
        if self.focused {
            draw_outline(cx, &mut self.draw_focus, rect, 1.5);
        }
        if control_access(self.enabled, self.focused).register_nav_stop {
            cx.add_nav_stop(self.draw_bg.area(), NavRole::TextInput, Inset::default());
        }
        DrawStep::done()
    }
}

fn draw_outline(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, width: f64) {
    draw.draw_abs(
        cx,
        Rect {
            pos: rect.pos,
            size: dvec2(rect.size.x, width),
        },
    );
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(rect.pos.x, rect.pos.y + rect.size.y - width),
            size: dvec2(rect.size.x, width),
        },
    );
    draw.draw_abs(
        cx,
        Rect {
            pos: rect.pos,
            size: dvec2(width, rect.size.y),
        },
    );
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(rect.pos.x + rect.size.x - width, rect.pos.y),
            size: dvec2(width, rect.size.y),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::SegmentedState;
    use waml::model::CardinalityVisibility::{All, Explicit, Off};

    #[test]
    fn cardinality_segments_cycle_in_display_order() {
        let mut state = SegmentedState::new(vec![Off, Explicit, All], Explicit);

        state.select_next();
        assert_eq!(state.selected(), All);
        state.select_next();
        assert_eq!(state.selected(), Off);
        state.select_previous();
        assert_eq!(state.selected(), All);
    }

    #[test]
    fn enabled_controls_are_tab_stops_and_retain_focus() {
        let access = super::control_access(true, true);

        assert!(access.register_nav_stop);
        assert!(access.retain_focus);
    }

    #[test]
    fn disabled_controls_are_skipped_and_drop_focus() {
        let access = super::control_access(false, true);

        assert!(!access.register_nav_stop);
        assert!(!access.retain_focus);
    }
}
