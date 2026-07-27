use crate::diagram_display::ResolvedDiagramDisplay;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::popup::base::PopupResult;
use crate::popup::select::{SelectItem, SelectLead};
use crate::property_controls::{SegmentItem, SegmentedControl, ToggleControl};
use crate::select_box::SelectBox;
use makepad_widgets::*;
use waml::model::CardinalityVisibility;
use waml::ops::DiagramDisplaySet;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.DiagramPropertiesBase = #(DiagramProperties::register_widget(vm))
    mod.widgets.DiagramProperties = set_type_default() do mod.widgets.DiagramPropertiesBase{
        width: Fill
        height: Fill
        flow: Down
        spacing: 0.0
        show_bg: true
        scroll_bars: ScrollBars {
            scroll_bar_y: ScrollBar {
                draw_bg +: {
                    size: 5.0
                    color: atlas.text_dim
                    color_hover: atlas.accent
                    color_drag: atlas.accent
                }
            }
        }
        draw_bg +: {
            color: atlas.surface
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }

        header := View {
            width: Fill
            height: 38.0
            flow: Right
            align: Align{y: 0.5}
            padding: Inset{left: 14.0, right: 8.0, top: 0.0, bottom: 0.0}

            heading := Label {
                text: "Diagram properties"
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_label
                }
            }
            header_spacer := View { width: Fill height: 1.0 }
            close := IconButton { width: 30.0 height: 30.0 }
        }

        top_rule := View {
            width: Fill
            height: 1.0
            show_bg: true
            draw_bg +: { color: atlas.surface_border }
        }

        body := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{x: 0.0}
            padding: Inset{left: 22.0, right: 22.0, top: 10.0, bottom: 10.0}

            form := View {
            width: Fill{max: 620.0}
            height: Fit
            flow: Down
            spacing: 4.0

            description_label := Label {
                text: "Note"
                margin: Inset{top: 2.0}
                draw_text +: { color: atlas.text text_style: fonts.text_menu }
            }
            description_input := TextInput {
                width: Fill
                height: Fit{min: FitBound.Abs(46), max: FitBound.Abs(100)}
                padding: Inset{left: 9.0, right: 9.0, top: 5.0, bottom: 5.0}
                is_multiline: true
                empty_text: "Optional note"
                draw_bg +: {
                    color: atlas.field_bg
                    color_hover: atlas.field_bg
                    color_focus: atlas.field_bg
                    color_down: atlas.field_bg
                    color_empty: atlas.field_bg
                    color_disabled: atlas.surface
                    border_color: atlas.surface_border
                    border_color_hover: atlas.frame_lo
                    border_color_focus: atlas.accent
                    border_color_down: atlas.accent
                    border_color_empty: atlas.surface_border
                    border_color_disabled: atlas.surface_border
                    border_radius: 3.0
                    border_size: 1.0
                }
                draw_text +: {
                    color: atlas.text
                    color_hover: atlas.text
                    color_focus: atlas.text
                    color_down: atlas.text
                    color_empty: atlas.text_dim
                    color_empty_hover: atlas.text_dim
                    color_empty_focus: atlas.text_dim
                    color_disabled: atlas.text_dim
                    text_style: fonts.text_menu
                }
                draw_cursor +: { color: atlas.accent }
                draw_selection +: {
                    color: atlas.frame_lo
                    color_hover: atlas.frame_lo
                    color_focus: atlas.frame_lo
                    color_down: atlas.frame_lo
                    color_empty: atlas.frame_lo
                    color_disabled: atlas.surface_border
                }
            }

            attributes_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 7.0, bottom: 3.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            attributes_section := Label {
                text: "Attributes"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            attributes_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                attributes_label := Label {
                    text: "Show attributes"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                attributes_spacer := View { width: Fill height: 1.0 }
                attributes_toggle := ToggleControl {}
            }
            types_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                types_label := Label {
                    text: "Show type"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                types_spacer := View { width: Fill height: 1.0 }
                types_toggle := ToggleControl {}
            }
            visibility_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                visibility_label := Label {
                    text: "Show visibility"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                visibility_spacer := View { width: Fill height: 1.0 }
                visibility_toggle := ToggleControl {}
            }
            attribute_cardinality_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                attribute_cardinality_label := Label {
                    text: "Cardinality"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                attribute_cardinality_spacer := View { width: Fill height: 1.0 }
                cardinality_control := SegmentedControl {
                    width: Fill { max: 280.0 }
                    height: 26.0
                    draw_label +: { text_style: fonts.text_menu }
                    draw_label_selected +: { text_style: fonts.text_menu }
                    draw_label_disabled +: { text_style: fonts.text_menu }
                }
            }
            max_attributes_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                max_attributes_label := Label {
                    text: "Max attributes"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                max_attributes_spacer := View { width: Fill height: 1.0 }
                max_attributes_select := SelectBox {
                    width: 72.0
                    height: 26.0
                    show_field: true
                    draw_label +: { text_style: fonts.text_menu }
                }
            }
            relationships_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 7.0, bottom: 3.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            relationships_section := Label {
                text: "Relationships"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            roles_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                roles_label := Label {
                    text: "Show roles"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                roles_spacer := View { width: Fill height: 1.0 }
                roles_toggle := ToggleControl {}
            }
            relationship_cardinality_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                relationship_cardinality_label := Label {
                    text: "Show cardinality"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                relationship_cardinality_spacer := View { width: Fill height: 1.0 }
                cardinality_toggle := ToggleControl {}
            }
            labels_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                labels_label := Label {
                    text: "Show labels"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                labels_spacer := View { width: Fill height: 1.0 }
                labels_toggle := ToggleControl {}
            }

            stereotypes_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 7.0, bottom: 3.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            stereotypes_section := Label {
                text: "Stereotypes"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            stereotypes_row := View {
                width: Fill height: 26.0 flow: Right align: Align{y: 0.5}
                stereotypes_label := Label {
                    text: "Show stereotype"
                    draw_text +: { color: atlas.text text_style: fonts.text_menu }
                }
                stereotypes_spacer := View { width: Fill height: 1.0 }
                stereotypes_toggle := ToggleControl {}
            }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramPropertiesAction {
    DisplayChanged(DiagramDisplaySet),
    DescriptionChanged(Option<String>),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyChange {
    ShowAttributes(bool),
    ShowType(bool),
    ShowAttributeVisibility(bool),
    MaxAttributes(Option<u32>),
    ShowRoles(bool),
    Cardinality(CardinalityVisibility),
    ShowCardinality(bool),
    ShowLabels(bool),
    ShowStereotype(bool),
    Description(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagramPropertiesState {
    description: Option<String>,
    display: ResolvedDiagramDisplay,
}

fn normalize_description(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let mut normalized = String::with_capacity(value.len());
        let mut previous_was_cr = false;
        for character in value.chars() {
            match character {
                '\r' => {
                    normalized.push('\n');
                    previous_was_cr = true;
                }
                '\n' if previous_was_cr => previous_was_cr = false,
                '\n' => normalized.push('\n'),
                _ => {
                    normalized.push(character);
                    previous_was_cr = false;
                }
            }
        }
        (!normalized.trim().is_empty()).then_some(normalized)
    })
}

impl DiagramPropertiesState {
    pub fn new(description: Option<String>, display: ResolvedDiagramDisplay) -> Self {
        Self {
            description,
            display,
        }
    }

    pub fn apply(&mut self, change: PropertyChange) -> DiagramPropertiesAction {
        match change {
            PropertyChange::ShowAttributes(value) => self.display.show_attributes = value,
            PropertyChange::ShowType(value) => self.display.show_type = value,
            PropertyChange::ShowAttributeVisibility(value) => {
                self.display.show_attribute_visibility = value
            }
            PropertyChange::MaxAttributes(value) => self.display.max_attributes = value,
            PropertyChange::ShowRoles(value) => self.display.show_roles = value,
            PropertyChange::Cardinality(value) => self.display.cardinality = value,
            PropertyChange::ShowCardinality(value) => self.display.show_cardinality = value,
            PropertyChange::ShowLabels(value) => self.display.show_labels = value,
            PropertyChange::ShowStereotype(value) => self.display.show_stereotype = value,
            PropertyChange::Description(value) => {
                self.description = normalize_description(value);
                return DiagramPropertiesAction::DescriptionChanged(self.description.clone());
            }
        }
        DiagramPropertiesAction::DisplayChanged(self.display_set())
    }

    fn display_set(&self) -> DiagramDisplaySet {
        DiagramDisplaySet {
            show_attributes: self.display.show_attributes,
            show_type: self.display.show_type,
            show_attribute_visibility: self.display.show_attribute_visibility,
            cardinality: self.display.cardinality,
            max_attributes: self.display.max_attributes,
            show_roles: self.display.show_roles,
            show_cardinality: self.display.show_cardinality,
            show_labels: self.display.show_labels,
            show_stereotype: self.display.show_stereotype,
            stereotype_filter: self.display.stereotype_filter.clone(),
            stereotype_colors: self.display.stereotype_colors.clone(),
        }
    }
}

fn cardinality_id(value: CardinalityVisibility) -> LiveId {
    match value {
        CardinalityVisibility::Off => live_id!(cardinality_off),
        CardinalityVisibility::Explicit => live_id!(cardinality_explicit),
        CardinalityVisibility::All => live_id!(cardinality_all),
    }
}

fn cardinality_from_id(id: LiveId) -> Option<CardinalityVisibility> {
    if id == live_id!(cardinality_off) {
        Some(CardinalityVisibility::Off)
    } else if id == live_id!(cardinality_explicit) {
        Some(CardinalityVisibility::Explicit)
    } else if id == live_id!(cardinality_all) {
        Some(CardinalityVisibility::All)
    } else {
        None
    }
}

fn cardinality_segments() -> Vec<SegmentItem> {
    vec![
        SegmentItem::new(live_id!(cardinality_all), "On"),
        SegmentItem::new(live_id!(cardinality_explicit), "Explicit"),
        SegmentItem::new(live_id!(cardinality_off), "Off"),
    ]
}

fn max_attributes_id(value: Option<u32>) -> LiveId {
    match value {
        None => live_id!(max_attributes_all),
        Some(1) => live_id!(max_attributes_1),
        Some(2) => live_id!(max_attributes_2),
        Some(3) => live_id!(max_attributes_3),
        Some(4) => live_id!(max_attributes_4),
        Some(5) => live_id!(max_attributes_5),
        Some(6) => live_id!(max_attributes_6),
        Some(7) => live_id!(max_attributes_7),
        Some(8) => live_id!(max_attributes_8),
        Some(9) => live_id!(max_attributes_9),
        Some(10) => live_id!(max_attributes_10),
        Some(_) => live_id!(max_attributes_all),
    }
}

fn max_attributes_from_id(id: LiveId) -> Option<Option<u32>> {
    [
        (live_id!(max_attributes_all), None),
        (live_id!(max_attributes_1), Some(1)),
        (live_id!(max_attributes_2), Some(2)),
        (live_id!(max_attributes_3), Some(3)),
        (live_id!(max_attributes_4), Some(4)),
        (live_id!(max_attributes_5), Some(5)),
        (live_id!(max_attributes_6), Some(6)),
        (live_id!(max_attributes_7), Some(7)),
        (live_id!(max_attributes_8), Some(8)),
        (live_id!(max_attributes_9), Some(9)),
        (live_id!(max_attributes_10), Some(10)),
    ]
    .into_iter()
    .find_map(|(candidate, value)| (candidate == id).then_some(value))
}

fn max_attribute_choices(selected: Option<u32>) -> Vec<SelectItem> {
    std::iter::once((None, "All".to_string()))
        .chain((1..=10).map(|value| (Some(value), value.to_string())))
        .map(|(value, label)| SelectItem {
            id: max_attributes_id(value),
            lead: SelectLead::None,
            label,
            selected: value == selected,
            enabled: true,
        })
        .collect()
}

fn max_attributes_index(value: Option<u32>) -> Option<usize> {
    match value {
        None => Some(0),
        Some(value) if (1..=10).contains(&value) => Some(value as usize),
        Some(_) => None,
    }
}

fn max_attributes_display_label(value: Option<u32>) -> Option<String> {
    value
        .filter(|value| !(1..=10).contains(value))
        .map(|value| value.to_string())
}

#[derive(Script, ScriptHook, Widget)]
pub struct DiagramProperties {
    #[deref]
    view: View,
    #[rust]
    state: Option<DiagramPropertiesState>,
}

impl DiagramProperties {
    pub fn set_diagram(
        &mut self,
        cx: &mut Cx,
        description: Option<&str>,
        display: &ResolvedDiagramDisplay,
    ) {
        let next = DiagramPropertiesState::new(description.map(str::to_string), display.clone());
        if self.state.as_ref() != Some(&next) {
            self.state = Some(next);
            self.view.redraw(cx);
        }
    }

    fn emit_change(&mut self, cx: &mut Cx, change: PropertyChange) {
        if let Some(state) = &mut self.state {
            let action = state.apply(change);
            cx.widget_action(self.widget_uid(), action);
            self.view.redraw(cx);
        }
    }

    fn max_attributes_open_request(
        &self,
        cx: &mut Cx,
        actions: &Actions,
    ) -> Option<(Rect, f64, Vec<SelectItem>)> {
        self.view
            .widget(cx, ids!(max_attributes_select))
            .borrow::<SelectBox>()
            .and_then(|control| control.open_request(actions))
    }

    fn max_attributes_closed(
        &mut self,
        cx: &mut Cx,
        result: PopupResult,
    ) -> Option<DiagramPropertiesAction> {
        let id = self
            .view
            .widget(cx, ids!(max_attributes_select))
            .borrow_mut::<SelectBox>()?
            .on_closed(cx, result)?;
        let value = max_attributes_from_id(id)?;
        let action = self
            .state
            .as_mut()?
            .apply(PropertyChange::MaxAttributes(value));
        self.view.redraw(cx);
        Some(action)
    }

    fn sync_controls(&mut self, cx: &mut Cx) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let description = state.description.as_deref().unwrap_or("");
        let description_input = self.view.text_input(cx, ids!(description_input));
        if description_input.text() != description {
            description_input.set_text(cx, description);
        }
        let max_attributes = state.display.max_attributes;
        if let Some(mut control) = self
            .view
            .widget(cx, ids!(max_attributes_select))
            .borrow_mut::<SelectBox>()
        {
            control.set_items(cx, max_attribute_choices(max_attributes));
            control.set_selected(cx, max_attributes_index(max_attributes));
            control.set_display_label(cx, max_attributes_display_label(max_attributes));
            control.set_enabled(cx, state.display.show_attributes);
        }

        for (path, value, enabled) in [
            (ids!(attributes_toggle), state.display.show_attributes, true),
            (
                ids!(types_toggle),
                state.display.show_type,
                state.display.show_attributes,
            ),
            (
                ids!(visibility_toggle),
                state.display.show_attribute_visibility,
                state.display.show_attributes,
            ),
            (
                ids!(stereotypes_toggle),
                state.display.show_stereotype,
                true,
            ),
            (ids!(roles_toggle), state.display.show_roles, true),
            (
                ids!(cardinality_toggle),
                state.display.show_cardinality,
                true,
            ),
            (ids!(labels_toggle), state.display.show_labels, true),
        ] {
            let widget = self.view.widget(cx, path);
            if let Some(mut control) = widget.borrow_mut::<ToggleControl>() {
                control.set_value(cx, value);
                control.set_enabled(cx, enabled);
            };
        }
        let segmented = self.view.widget(cx, ids!(cardinality_control));
        if let Some(mut control) = segmented.borrow_mut::<SegmentedControl>() {
            control.set_items(cx, cardinality_segments());
            control.set_selected(cx, cardinality_id(state.display.cardinality));
        }
        self.view
            .widget(cx, ids!(close))
            .as_icon_button()
            .set_icon(cx, Icon::CircleX);
    }
}

impl Widget for DiagramProperties {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.sync_controls(cx);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for DiagramProperties {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        if self
            .view
            .widget(cx, ids!(close))
            .as_icon_button()
            .clicked(actions)
        {
            cx.widget_action(self.widget_uid(), DiagramPropertiesAction::Close);
        }

        if let Some(description) = self
            .view
            .text_input(cx, ids!(description_input))
            .changed(actions)
        {
            self.emit_change(
                cx,
                PropertyChange::Description(if description.trim().is_empty() {
                    None
                } else {
                    Some(description)
                }),
            );
        }
        for (path, to_change) in [
            (
                ids!(attributes_toggle),
                PropertyChange::ShowAttributes as fn(bool) -> PropertyChange,
            ),
            (ids!(types_toggle), PropertyChange::ShowType),
            (
                ids!(visibility_toggle),
                PropertyChange::ShowAttributeVisibility,
            ),
            (ids!(stereotypes_toggle), PropertyChange::ShowStereotype),
            (ids!(roles_toggle), PropertyChange::ShowRoles),
            (ids!(cardinality_toggle), PropertyChange::ShowCardinality),
            (ids!(labels_toggle), PropertyChange::ShowLabels),
        ] {
            let changed = self
                .view
                .widget(cx, path)
                .borrow::<ToggleControl>()
                .and_then(|control| control.changed(actions));
            if let Some(value) = changed {
                self.emit_change(cx, to_change(value));
            }
        }

        let cardinality = self
            .view
            .widget(cx, ids!(cardinality_control))
            .borrow::<SegmentedControl>()
            .and_then(|control| control.changed(actions))
            .and_then(cardinality_from_id);
        if let Some(value) = cardinality {
            self.emit_change(cx, PropertyChange::Cardinality(value));
        }
    }
}

impl DiagramPropertiesRef {
    pub fn set_diagram(
        &self,
        cx: &mut Cx,
        description: Option<&str>,
        display: &ResolvedDiagramDisplay,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_diagram(cx, description, display);
        }
    }

    pub fn max_attributes_open_request(
        &self,
        cx: &mut Cx,
        actions: &Actions,
    ) -> Option<(Rect, f64, Vec<SelectItem>)> {
        self.borrow()?.max_attributes_open_request(cx, actions)
    }

    pub fn max_attributes_closed(
        &self,
        cx: &mut Cx,
        result: PopupResult,
    ) -> Option<DiagramPropertiesAction> {
        self.borrow_mut()?.max_attributes_closed(cx, result)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiagramProperties, DiagramPropertiesAction, DiagramPropertiesState, PropertyChange,
    };
    use crate::diagram_display::ResolvedDiagramDisplay;
    use crate::property_controls::SegmentedControl;
    use crate::select_box::SelectBox;
    use makepad_widgets::{
        ids, live_id, script_eval, Apply, FitBound, Label, LiveId, Scope, ScriptApply, ScriptMod,
        ScriptNew, ScriptVmCx, Size, TextInput, Widget, WidgetRef,
    };
    use waml::model::CardinalityVisibility;
    use waml::ops::DiagramDisplaySet;

    fn resolved_display() -> ResolvedDiagramDisplay {
        ResolvedDiagramDisplay {
            show_attributes: true,
            show_type: true,
            show_attribute_visibility: false,
            cardinality: CardinalityVisibility::Explicit,
            max_attributes: Some(7),
            show_roles: false,
            show_cardinality: true,
            show_labels: true,
            show_stereotype: false,
            stereotype_filter: Some(vec!["entity".into()]),
            stereotype_colors: vec!["entity=#1496dc".into()],
        }
    }

    fn scripted_properties() -> (makepad_widgets::ScriptVm<'static>, WidgetRef) {
        let mut vm = crate::script_gate::boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);
        crate::fonts::script_mod(&mut vm);
        crate::icons::script_mod(&mut vm);
        crate::icon_button::script_mod(&mut vm);
        crate::property_controls::script_mod(&mut vm);
        crate::select_box::script_mod(&mut vm);
        crate::diagram_properties::script_mod(&mut vm);
        let value = script_eval!(vm, { mod.widgets.DiagramProperties {} });
        let mut widget = WidgetRef::script_new(&mut vm);
        widget.script_apply(&mut vm, &Apply::New, &mut Scope::empty(), value);
        (vm, widget)
    }

    #[test]
    fn form_uses_bounded_fill_for_responsive_left_anchored_layout() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();

        let walk = properties.view.widget(cx, ids!(form)).walk(cx);

        match walk.width {
            Size::Fill { max, .. } => assert_eq!(max, Some(620.0)),
            other => panic!("form width must be bounded Fill, got {other:?}"),
        }
    }

    #[test]
    fn note_input_is_a_bounded_fit_height_multiline_editor() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();
        let input = properties.view.widget(cx, ids!(description_input));

        assert!(input
            .borrow::<TextInput>()
            .expect("description input")
            .is_multiline());
        match input.walk(cx).height {
            Size::Fit {
                min: Some(FitBound::Abs(min)),
                max: Some(FitBound::Abs(max)),
            } => {
                assert_eq!(min, 46.0);
                assert_eq!(max, 100.0);
            }
            other => panic!("Note height must be Fit from three rows to 100px, got {other:?}"),
        }
    }

    #[test]
    fn identity_section_heading_is_not_rendered() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();

        assert!(properties
            .view
            .widget(cx, ids!(identity_section))
            .borrow::<Label>()
            .is_none());
    }

    #[test]
    fn note_input_uses_compact_text_selection_and_default_scrollbar() {
        let source = include_str!("diagram_properties.rs");
        let note = source
            .split_once("description_input := TextInput {")
            .and_then(|(_, tail)| tail.split_once("attributes_rule := View {"))
            .map(|(block, _)| block)
            .expect("Note input DSL block");

        assert!(note.contains("text_style: fonts.text_menu"));
        assert!(
            note.matches("atlas.frame_lo").count() >= 5,
            "selection idle/hover/focus/down/empty colors must all remain visible"
        );
        assert!(
            !note.contains("scroll_bar: ScrollBar {"),
            "Note should inherit Makepad's stock multiline TextInput scrollbar"
        );
    }

    #[test]
    fn cardinality_control_is_bounded_and_can_shrink() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();

        let walk = properties
            .view
            .widget(cx, ids!(cardinality_control))
            .walk(cx);
        match walk.width {
            Size::Fill { max, .. } => assert_eq!(max, Some(280.0)),
            other => panic!("cardinality width must be bounded Fill, got {other:?}"),
        }
    }

    #[test]
    fn property_and_cardinality_labels_use_compact_panel_typography() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();

        let note_caption = properties.view.widget(cx, ids!(description_label));
        assert_eq!(
            note_caption
                .borrow::<Label>()
                .expect("Note caption")
                .draw_text
                .text_style
                .font_size,
            10.0
        );

        let heading = properties.view.widget(cx, ids!(heading));
        assert_eq!(
            heading
                .borrow::<Label>()
                .expect("page heading")
                .draw_text
                .text_style
                .font_size,
            11.0
        );
        assert!(matches!(
            properties.view.widget(cx, ids!(header)).walk(cx).height,
            Size::Fixed(38.0)
        ));

        let row_label = properties.view.widget(cx, ids!(attributes_label));
        assert_eq!(
            row_label
                .borrow::<Label>()
                .expect("property row label")
                .draw_text
                .text_style
                .font_size,
            10.0
        );

        let control = properties.view.widget(cx, ids!(cardinality_control));
        assert_eq!(
            control
                .borrow::<SegmentedControl>()
                .expect("cardinality control")
                .label_font_sizes(),
            [10.0; 3]
        );
    }

    #[test]
    fn title_editor_is_absent_because_diagram_naming_is_owned_elsewhere() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();

        assert!(properties
            .view
            .widget(cx, ids!(title_label))
            .borrow::<Label>()
            .is_none());
        assert!(properties
            .view
            .widget(cx, ids!(title_input))
            .borrow::<TextInput>()
            .is_none());
    }

    #[test]
    fn max_attributes_is_a_compact_bounded_select_box() {
        let (mut vm, widget) = scripted_properties();
        let properties = widget
            .borrow::<DiagramProperties>()
            .expect("DiagramProperties widget");
        let cx = vm.cx_mut();
        let control = properties.view.widget(cx, ids!(max_attributes_select));

        assert!(control
            .borrow::<SelectBox>()
            .is_some_and(|control| control.shows_field()));
        let walk = control.walk(cx);
        assert!(matches!(walk.width, Size::Fixed(72.0)));
        assert!(matches!(walk.height, Size::Fixed(26.0)));
    }

    #[test]
    fn changing_one_property_emits_the_complete_display() {
        let mut state = DiagramPropertiesState::new(Some("Flow".into()), resolved_display());

        let action = state.apply(PropertyChange::ShowType(false));

        assert_eq!(
            action,
            DiagramPropertiesAction::DisplayChanged(DiagramDisplaySet {
                show_attributes: true,
                show_type: false,
                show_attribute_visibility: false,
                cardinality: CardinalityVisibility::Explicit,
                max_attributes: Some(7),
                show_roles: false,
                show_cardinality: true,
                show_labels: true,
                show_stereotype: false,
                stereotype_filter: Some(vec!["entity".into()]),
                stereotype_colors: vec!["entity=#1496dc".into()],
            })
        );
    }

    #[test]
    fn changing_cardinality_emits_the_selected_enum() {
        let mut state = DiagramPropertiesState::new(None, resolved_display());

        let action = state.apply(PropertyChange::Cardinality(CardinalityVisibility::All));

        let DiagramPropertiesAction::DisplayChanged(display) = action else {
            panic!("cardinality changes must emit a display payload");
        };
        assert_eq!(display.cardinality, CardinalityVisibility::All);
        assert!(
            display.show_cardinality,
            "changing attribute cardinality must preserve relationship cardinality"
        );
    }

    #[test]
    fn changing_relationship_cardinality_preserves_the_attribute_mode() {
        let mut state = DiagramPropertiesState::new(None, resolved_display());

        let action = state.apply(PropertyChange::ShowCardinality(false));

        let DiagramPropertiesAction::DisplayChanged(display) = action else {
            panic!("relationship cardinality changes must emit a display payload");
        };
        assert!(!display.show_cardinality);
        assert_eq!(display.cardinality, CardinalityVisibility::Explicit);
    }

    #[test]
    fn clearing_description_emits_only_the_description() {
        let mut state = DiagramPropertiesState::new(Some("Flow".into()), resolved_display());

        let action = state.apply(PropertyChange::Description(None));

        assert_eq!(action, DiagramPropertiesAction::DescriptionChanged(None));
    }

    #[test]
    fn description_change_preserves_lines_and_normalizes_them_to_lf() {
        let mut state = DiagramPropertiesState::new(None, resolved_display());

        let action = state.apply(PropertyChange::Description(Some(
            "First line\r\nSecond line\rThird line\nFourth line".into(),
        )));

        assert_eq!(
            action,
            DiagramPropertiesAction::DescriptionChanged(Some(
                "First line\nSecond line\nThird line\nFourth line".into()
            ))
        );
    }

    #[test]
    fn cardinality_segment_ids_map_to_the_shared_enum() {
        assert_eq!(
            super::cardinality_from_id(live_id!(cardinality_off)),
            Some(CardinalityVisibility::Off)
        );
        assert_eq!(
            super::cardinality_from_id(live_id!(cardinality_explicit)),
            Some(CardinalityVisibility::Explicit)
        );
        assert_eq!(
            super::cardinality_from_id(live_id!(cardinality_all)),
            Some(CardinalityVisibility::All)
        );
        assert_eq!(super::cardinality_from_id(live_id!(unknown)), None);
    }

    #[test]
    fn attribute_cardinality_segments_use_the_visible_on_explicit_off_order() {
        let items = super::cardinality_segments();
        assert_eq!(
            items
                .iter()
                .map(|item| (item.id, item.label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (live_id!(cardinality_all), "On"),
                (live_id!(cardinality_explicit), "Explicit"),
                (live_id!(cardinality_off), "Off"),
            ]
        );
    }

    #[test]
    fn maximum_attribute_choices_are_all_then_one_through_ten() {
        let choices = super::max_attribute_choices(Some(7));
        assert_eq!(
            choices
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["All", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]
        );
        assert!(choices[7].selected);
        assert_eq!(
            super::max_attributes_from_id(live_id!(max_attributes_all)),
            Some(None)
        );
        assert_eq!(
            super::max_attributes_from_id(live_id!(max_attributes_1)),
            Some(Some(1))
        );
        assert_eq!(
            super::max_attributes_from_id(live_id!(max_attributes_10)),
            Some(Some(10))
        );
        assert_eq!(super::max_attributes_from_id(live_id!(not_a_maximum)), None);
    }

    #[test]
    fn authored_maximum_above_picker_range_is_not_mapped_to_all() {
        let choices = super::max_attribute_choices(Some(12));

        assert_eq!(choices.len(), 11);
        assert!(!choices.iter().any(|item| item.selected));
        assert_eq!(super::max_attributes_index(Some(12)), None);
        assert_eq!(
            super::max_attributes_display_label(Some(12)).as_deref(),
            Some("12")
        );
    }
}
