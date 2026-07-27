use crate::diagram_display::ResolvedDiagramDisplay;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::property_controls::{SegmentItem, SegmentedControl, ToggleControl};
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
        width: 320.0
        height: Fit
        flow: Down
        spacing: 0.0
        show_bg: true
        scroll_bars: ScrollBars{scroll_bar_y: ScrollBar{}}
        draw_bg +: {
            color: atlas.surface
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }

        header := View {
            width: Fill
            height: 44.0
            flow: Right
            align: Align{y: 0.5}
            padding: Inset{left: 14.0, right: 8.0, top: 0.0, bottom: 0.0}

            heading := Label {
                text: "Diagram properties"
                draw_text +: {
                    color: atlas.text
                    text_style: fonts.text_heading
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
            flow: Down
            spacing: 5.0
            padding: Inset{left: 14.0, right: 14.0, top: 10.0, bottom: 10.0}

            identity_section := Label {
                text: "Identity"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            title_label := Label {
                text: "Title"
                draw_text +: { color: atlas.text text_style: fonts.text_label }
            }
            title_input := TextInput {
                width: Fill
                height: 30.0
                padding: Inset{left: 9.0, right: 9.0, top: 5.0, bottom: 5.0}
                empty_text: "Diagram title"
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
                    text_style: fonts.text_body
                }
                draw_cursor +: { color: atlas.accent }
                draw_selection +: { color: atlas.selection }
            }
            description_label := Label {
                text: "Note"
                margin: Inset{top: 2.0}
                draw_text +: { color: atlas.text text_style: fonts.text_label }
            }
            description_input := TextInput {
                width: Fill
                height: 42.0
                padding: Inset{left: 9.0, right: 9.0, top: 5.0, bottom: 5.0}
                is_multiline: false
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
                    text_style: fonts.text_body
                }
                draw_cursor +: { color: atlas.accent }
                draw_selection +: { color: atlas.selection }
            }

            attributes_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 8.0, bottom: 4.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            attributes_section := Label {
                text: "Attributes"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            attributes_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                attributes_label := Label {
                    text: "Show attributes"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                attributes_spacer := View { width: Fill height: 1.0 }
                attributes_toggle := ToggleControl {}
            }
            types_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                types_label := Label {
                    text: "Show type"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                types_spacer := View { width: Fill height: 1.0 }
                types_toggle := ToggleControl {}
            }
            visibility_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                visibility_label := Label {
                    text: "Show visibility"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                visibility_spacer := View { width: Fill height: 1.0 }
                visibility_toggle := ToggleControl {}
            }
            attribute_cardinality_row := View {
                width: Fill height: 32.0 flow: Right align: Align{y: 0.5}
                attribute_cardinality_label := Label {
                    text: "Cardinality"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                attribute_cardinality_spacer := View { width: Fill height: 1.0 }
                cardinality_control := SegmentedControl {}
            }
            max_attributes_row := View {
                width: Fill height: 32.0 flow: Right align: Align{y: 0.5}
                max_attributes_label := Label {
                    text: "Max attributes"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                max_attributes_spacer := View { width: Fill height: 1.0 }
                max_attributes_input := TextInput {
                    width: 72.0
                    height: 30.0
                    padding: Inset{left: 9.0, right: 9.0, top: 5.0, bottom: 5.0}
                    is_numeric_only: true
                    empty_text: "All"
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
                        text_style: fonts.text_body
                    }
                    draw_cursor +: { color: atlas.accent }
                    draw_selection +: { color: atlas.selection }
                }
            }
            relationships_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 8.0, bottom: 4.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            relationships_section := Label {
                text: "Relationships"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            roles_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                roles_label := Label {
                    text: "Show roles"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                roles_spacer := View { width: Fill height: 1.0 }
                roles_toggle := ToggleControl {}
            }
            relationship_cardinality_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                relationship_cardinality_label := Label {
                    text: "Show cardinality"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                relationship_cardinality_spacer := View { width: Fill height: 1.0 }
                cardinality_toggle := ToggleControl {}
            }
            labels_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                labels_label := Label {
                    text: "Show labels"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                labels_spacer := View { width: Fill height: 1.0 }
                labels_toggle := ToggleControl {}
            }

            stereotypes_rule := View {
                width: Fill height: 1.0
                margin: Inset{top: 8.0, bottom: 4.0}
                show_bg: true
                draw_bg +: { color: atlas.surface_border }
            }
            stereotypes_section := Label {
                text: "Stereotypes"
                draw_text +: { color: atlas.text_dim text_style: fonts.text_eyebrow }
            }
            stereotypes_row := View {
                width: Fill height: 28.0 flow: Right align: Align{y: 0.5}
                stereotypes_label := Label {
                    text: "Show stereotype"
                    draw_text +: { color: atlas.text text_style: fonts.text_body }
                }
                stereotypes_spacer := View { width: Fill height: 1.0 }
                stereotypes_toggle := ToggleControl {}
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramPropertiesAction {
    DisplayChanged(DiagramDisplaySet),
    IdentityChanged {
        title: String,
        description: Option<String>,
    },
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
    Title(String),
    Description(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagramPropertiesState {
    title: String,
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
    pub fn new(
        title: String,
        description: Option<String>,
        display: ResolvedDiagramDisplay,
    ) -> Self {
        Self {
            title,
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
            PropertyChange::Title(value) => {
                self.title = value;
                return self.identity_action();
            }
            PropertyChange::Description(value) => {
                self.description = normalize_description(value);
                return self.identity_action();
            }
        }
        DiagramPropertiesAction::DisplayChanged(self.display_set())
    }

    fn identity_action(&self) -> DiagramPropertiesAction {
        DiagramPropertiesAction::IdentityChanged {
            title: self.title.clone(),
            description: self.description.clone(),
        }
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

fn max_attributes_from_text(text: &str) -> Option<Option<u32>> {
    let text = text.trim();
    if text.is_empty() {
        Some(None)
    } else {
        text.parse().ok().map(Some)
    }
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
        title: &str,
        description: Option<&str>,
        display: &ResolvedDiagramDisplay,
    ) {
        let next = DiagramPropertiesState::new(
            title.to_string(),
            description.map(str::to_string),
            display.clone(),
        );
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

    fn sync_controls(&mut self, cx: &mut Cx) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let title_input = self.view.text_input(cx, ids!(title_input));
        if title_input.text() != state.title {
            title_input.set_text(cx, &state.title);
        }
        let description = state.description.as_deref().unwrap_or("");
        let description_input = self.view.text_input(cx, ids!(description_input));
        if description_input.text() != description {
            description_input.set_text(cx, description);
        }
        let max_attributes = state
            .display
            .max_attributes
            .map(|value| value.to_string())
            .unwrap_or_default();
        let max_input = self.view.text_input(cx, ids!(max_attributes_input));
        if max_input.text() != max_attributes {
            max_input.set_text(cx, &max_attributes);
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
        self.view
            .widget(cx, ids!(max_attributes_input))
            .set_disabled(cx, !state.display.show_attributes);

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

        if let Some(title) = self.view.text_input(cx, ids!(title_input)).changed(actions) {
            self.emit_change(cx, PropertyChange::Title(title));
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
        if let Some(text) = self
            .view
            .text_input(cx, ids!(max_attributes_input))
            .changed(actions)
        {
            if let Some(value) = max_attributes_from_text(&text) {
                self.emit_change(cx, PropertyChange::MaxAttributes(value));
            }
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
        title: &str,
        description: Option<&str>,
        display: &ResolvedDiagramDisplay,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_diagram(cx, title, description, display);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagramPropertiesAction, DiagramPropertiesState, PropertyChange};
    use crate::diagram_display::ResolvedDiagramDisplay;
    use makepad_widgets::{live_id, LiveId};
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

    #[test]
    fn changing_one_property_emits_the_complete_display() {
        let mut state =
            DiagramPropertiesState::new("Orders".into(), Some("Flow".into()), resolved_display());

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
        let mut state = DiagramPropertiesState::new("Orders".into(), None, resolved_display());

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
        let mut state = DiagramPropertiesState::new("Orders".into(), None, resolved_display());

        let action = state.apply(PropertyChange::ShowCardinality(false));

        let DiagramPropertiesAction::DisplayChanged(display) = action else {
            panic!("relationship cardinality changes must emit a display payload");
        };
        assert!(!display.show_cardinality);
        assert_eq!(display.cardinality, CardinalityVisibility::Explicit);
    }

    #[test]
    fn changing_identity_emits_both_editable_fields() {
        let mut state =
            DiagramPropertiesState::new("Orders".into(), Some("Flow".into()), resolved_display());

        let action = state.apply(PropertyChange::Description(None));

        assert_eq!(
            action,
            DiagramPropertiesAction::IdentityChanged {
                title: "Orders".into(),
                description: None,
            }
        );
    }

    #[test]
    fn description_change_preserves_lines_and_normalizes_them_to_lf() {
        let mut state = DiagramPropertiesState::new("Orders".into(), None, resolved_display());

        let action = state.apply(PropertyChange::Description(Some(
            "First line\r\nSecond line\rThird line\nFourth line".into(),
        )));

        assert_eq!(
            action,
            DiagramPropertiesAction::IdentityChanged {
                title: "Orders".into(),
                description: Some(
                    "First line\nSecond line\nThird line\nFourth line".into()
                ),
            }
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
    fn maximum_attributes_parses_blank_as_unlimited() {
        assert_eq!(super::max_attributes_from_text(""), Some(None));
        assert_eq!(super::max_attributes_from_text("  "), Some(None));
        assert_eq!(super::max_attributes_from_text("12"), Some(Some(12)));
        assert_eq!(super::max_attributes_from_text("many"), None);
    }
}
