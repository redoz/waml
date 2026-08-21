use crate::error::OperationFailure;
use crate::{DiagramName, ViewKind};
use makepad_test::{Selector, TestApp, TestError, WidgetSnapshot};

const VIEW_BUTTON_ID: &str = "view_button";
const CANVAS_ID: &str = "canvas_wrap";
const SOURCE_ID: &str = "markdown_surface";

pub(crate) fn ensure_diagram_open(
    driver: &TestApp,
    diagram: DiagramName,
) -> Result<String, OperationFailure> {
    let initial = snapshot(
        driver,
        format!("{} diagram state could not be observed", diagram.display),
    )?;
    match decide_diagram_open(&initial, diagram)? {
        DiagramOpenDecision::AlreadyActive => {
            return Ok(format!(
                "{} diagram is active in Diagram view",
                diagram.display
            ));
        }
        DiagramOpenDecision::ClickRow => {}
    }

    let row = driver.locator(diagram_row_selector(diagram));
    row.try_click()
        .map_err(|error| diagram_driver_failure(driver, diagram, &error))?;
    row.try_wait_checked(true)
        .map_err(|error| diagram_driver_failure(driver, diagram, &error))?;
    wait_for_view(driver, ViewKind::Diagram)?;

    let observed = snapshot(
        driver,
        format!(
            "{} diagram postcondition could not be observed",
            diagram.display
        ),
    )?;
    observe_active_diagram(&observed, diagram)?;
    observe_active_view(&observed, ViewKind::Diagram)?;
    Ok(format!(
        "{} diagram is active in Diagram view",
        diagram.display
    ))
}

pub(crate) fn expect_active_diagram(
    driver: &TestApp,
    diagram: DiagramName,
) -> Result<String, OperationFailure> {
    let widgets = snapshot(
        driver,
        format!("{} diagram state could not be observed", diagram.display),
    )?;
    observe_active_diagram(&widgets, diagram)
}

pub(crate) fn switch_active_document_to(
    driver: &TestApp,
    requested: ViewKind,
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver, "document view state could not be observed")?;
    require_view_switch_is_a_change(&widgets, requested)?;

    let button = driver.locator(view_button_selector());
    let button_snapshot = button
        .try_snapshot()
        .map_err(|error| view_driver_failure(driver, &error))?;
    if !button_snapshot.enabled {
        return Err(OperationFailure {
            observed: "the document view switch is disabled".to_string(),
            detail: format!("cannot switch to {} view", view_name(requested)),
        });
    }
    button
        .try_click()
        .map_err(|error| view_driver_failure(driver, &error))?;
    wait_for_view(driver, requested)?;

    let observed = snapshot(driver, "document view postcondition could not be observed")?;
    observe_active_view(&observed, requested)
}

pub(crate) fn expect_active_view(
    driver: &TestApp,
    requested: ViewKind,
) -> Result<String, OperationFailure> {
    let widgets = snapshot(driver, "document view state could not be observed")?;
    observe_active_view(&widgets, requested)
}

fn observe_active_diagram(
    widgets: &[WidgetSnapshot],
    diagram: DiagramName,
) -> Result<String, OperationFailure> {
    let row = resolve_diagram_row(widgets, diagram)?;
    if row.checked != Some(true) {
        return Err(OperationFailure {
            observed: format!("{} row is not checked", diagram.display),
            detail: format!(
                "expected checked state `true`, found `{}`",
                row.checked
                    .map(|checked| checked.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ),
        });
    }

    let state = observed_view_state(widgets)?;
    match state {
        ObservedViewState::Diagram => {}
        ObservedViewState::BothVisible => {
            return Err(OperationFailure {
                observed: state.description(),
                detail: "an active diagram requires only the canvas surface to be visible"
                    .to_string(),
            });
        }
        ObservedViewState::Source | ObservedViewState::BothHidden => {
            return Err(OperationFailure {
                observed: format!(
                    "{} row is checked but its canvas is hidden",
                    diagram.display
                ),
                detail: format!("observed {}", state.description()),
            });
        }
    }

    Ok(format!(
        "{} diagram is active with a visible canvas",
        diagram.display
    ))
}

fn observe_active_view(
    widgets: &[WidgetSnapshot],
    requested: ViewKind,
) -> Result<String, OperationFailure> {
    let state = observed_view_state(widgets)?;
    match (state, requested) {
        (ObservedViewState::Diagram, ViewKind::Diagram) => Ok("Diagram view is active".to_string()),
        (ObservedViewState::Source, ViewKind::Source) => Ok("Source view is active".to_string()),
        (ObservedViewState::Diagram, ViewKind::Source) => Err(OperationFailure {
            observed: "Diagram view is active".to_string(),
            detail: "expected Source view".to_string(),
        }),
        (ObservedViewState::Source, ViewKind::Diagram) => Err(OperationFailure {
            observed: "Source view is active".to_string(),
            detail: "expected Diagram view".to_string(),
        }),
        (state, _) => Err(OperationFailure {
            observed: state.description(),
            detail: "exactly one document surface must be visible".to_string(),
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagramOpenDecision {
    AlreadyActive,
    ClickRow,
}

fn decide_diagram_open(
    widgets: &[WidgetSnapshot],
    diagram: DiagramName,
) -> Result<DiagramOpenDecision, OperationFailure> {
    resolve_diagram_row(widgets, diagram)?;
    let state = observed_view_state(widgets)?;
    match state {
        ObservedViewState::Diagram if observe_active_diagram(widgets, diagram).is_ok() => {
            Ok(DiagramOpenDecision::AlreadyActive)
        }
        ObservedViewState::Diagram | ObservedViewState::Source => Ok(DiagramOpenDecision::ClickRow),
        state => Err(OperationFailure {
            observed: state.description(),
            detail: format!(
                "cannot ensure {} is open from an invalid document surface state",
                diagram.display
            ),
        }),
    }
}

/// `switch_active_document_to` is imperative: it must move the active view,
/// so a request for the view that is already active is a scenario bug, not a
/// no-op. Every other surface state means the toggle cannot be trusted.
fn require_view_switch_is_a_change(
    widgets: &[WidgetSnapshot],
    requested: ViewKind,
) -> Result<(), OperationFailure> {
    let state = observed_view_state(widgets)?;
    match (state, requested) {
        (ObservedViewState::Diagram, ViewKind::Diagram)
        | (ObservedViewState::Source, ViewKind::Source) => Err(OperationFailure {
            observed: format!("{} view is already active", view_name(requested)),
            detail: "switch_active_document_to must change the active view".to_string(),
        }),
        (ObservedViewState::Diagram, ViewKind::Source)
        | (ObservedViewState::Source, ViewKind::Diagram) => Ok(()),
        (state, _) => Err(OperationFailure {
            observed: state.description(),
            detail: format!(
                "cannot switch to {} from an invalid document surface state",
                view_name(requested)
            ),
        }),
    }
}

fn resolve_diagram_row(
    widgets: &[WidgetSnapshot],
    diagram: DiagramName,
) -> Result<&WidgetSnapshot, OperationFailure> {
    let rows: Vec<_> = widgets
        .iter()
        .filter(|widget| {
            widget.visible
                && widget.widget_type == "WamlProjectTreeRow"
                && widget.text.as_deref() == Some(diagram.display)
        })
        .collect();
    let row = match rows.as_slice() {
        [] => {
            return Err(OperationFailure {
                observed: format!("no visible {} diagram row", diagram.display),
                detail: "expected exactly one visible enabled diagram row".to_string(),
            })
        }
        [row] => *row,
        rows => {
            return Err(OperationFailure {
                observed: format!("{} visible rows for {}", rows.len(), diagram.display),
                detail: "the diagram row is ambiguous".to_string(),
            })
        }
    };
    if !row.enabled {
        return Err(OperationFailure {
            observed: format!("{} diagram row is disabled", diagram.display),
            detail: "the diagram row must be enabled".to_string(),
        });
    }
    if row.value.as_deref() != Some(diagram.value) {
        return Err(OperationFailure {
            observed: format!("{} row has an invalid semantic value", diagram.display),
            detail: format!(
                "expected `{}`, found `{}`",
                diagram.value,
                row.value.as_deref().unwrap_or("<none>")
            ),
        });
    }
    Ok(row)
}

#[derive(Clone, Copy)]
enum ObservedViewState {
    Diagram,
    Source,
    BothVisible,
    BothHidden,
}

impl ObservedViewState {
    fn description(self) -> String {
        match self {
            Self::Diagram => "Diagram view is active".to_string(),
            Self::Source => "Source view is active".to_string(),
            Self::BothVisible => {
                "invalid document surface state: Diagram and Source are visible".to_string()
            }
            Self::BothHidden => {
                "invalid document surface state: Diagram and Source are hidden".to_string()
            }
        }
    }
}

fn observed_view_state(widgets: &[WidgetSnapshot]) -> Result<ObservedViewState, OperationFailure> {
    let canvas = surface_visibility(widgets, CANVAS_ID, "Diagram")?;
    let source = surface_visibility(widgets, SOURCE_ID, "Source")?;
    Ok(match (canvas, source) {
        (true, false) => ObservedViewState::Diagram,
        (false, true) => ObservedViewState::Source,
        (true, true) => ObservedViewState::BothVisible,
        (false, false) => ObservedViewState::BothHidden,
    })
}

fn surface_visibility(
    widgets: &[WidgetSnapshot],
    id: &str,
    semantic_name: &str,
) -> Result<bool, OperationFailure> {
    let surfaces: Vec<_> = widgets.iter().filter(|widget| widget.id == id).collect();
    match surfaces.as_slice() {
        [] => Err(OperationFailure {
            observed: format!(
                "invalid document surface state: {} surface is missing",
                semantic_name
            ),
            detail: "expected exactly one document surface snapshot".to_string(),
        }),
        [surface] => Ok(surface.visible),
        surfaces => Err(OperationFailure {
            observed: format!(
                "invalid document surface state: {} {} surfaces exist",
                surfaces.len(),
                semantic_name
            ),
            detail: "a document surface selector is ambiguous".to_string(),
        }),
    }
}

fn wait_for_view(driver: &TestApp, requested: ViewKind) -> Result<(), OperationFailure> {
    let (visible, hidden) = match requested {
        ViewKind::Diagram => (canvas_selector(), source_selector()),
        ViewKind::Source => (source_selector(), canvas_selector()),
    };
    driver
        .locator(visible)
        .try_wait_visible()
        .map_err(|error| view_driver_failure(driver, &error))?;
    driver
        .locator(hidden)
        .try_wait_hidden()
        .map_err(|error| view_driver_failure(driver, &error))?;
    Ok(())
}

fn snapshot(
    driver: &TestApp,
    observed: impl Into<String>,
) -> Result<Vec<WidgetSnapshot>, OperationFailure> {
    driver
        .try_widget_snapshot()
        .map_err(|error| OperationFailure {
            observed: observed.into(),
            detail: error.message().to_string(),
        })
}

fn diagram_driver_failure(
    driver: &TestApp,
    diagram: DiagramName,
    error: &TestError,
) -> OperationFailure {
    let observed = driver
        .try_widget_snapshot()
        .ok()
        .and_then(|widgets| observe_active_diagram(&widgets, diagram).err())
        .map(|failure| failure.observed)
        .unwrap_or_else(|| format!("{} diagram state could not be observed", diagram.display));
    OperationFailure {
        observed,
        detail: error.message().to_string(),
    }
}

fn view_driver_failure(driver: &TestApp, error: &TestError) -> OperationFailure {
    let observed = driver
        .try_widget_snapshot()
        .ok()
        .and_then(|widgets| observed_view_state(&widgets).ok())
        .map(ObservedViewState::description)
        .unwrap_or_else(|| "document view state could not be observed".to_string());
    OperationFailure {
        observed,
        detail: error.message().to_string(),
    }
}

fn diagram_row_selector(diagram: DiagramName) -> Selector {
    Selector::widget_type("WamlProjectTreeRow").text_exact(diagram.display)
}

fn view_button_selector() -> Selector {
    Selector::id(VIEW_BUTTON_ID)
}

fn canvas_selector() -> Selector {
    Selector::id(CANVAS_ID)
}

fn source_selector() -> Selector {
    Selector::id(SOURCE_ID)
}

fn view_name(view: ViewKind) -> &'static str {
    match view {
        ViewKind::Diagram => "Diagram",
        ViewKind::Source => "Source",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decide_diagram_open, observe_active_diagram, observe_active_view,
        require_view_switch_is_a_change, DiagramOpenDecision,
    };
    use crate::{DiagramName, ViewKind};
    use makepad_test::WidgetSnapshot;

    fn snapshot(id: &str, widget_type: &str, visible: bool) -> WidgetSnapshot {
        WidgetSnapshot {
            id: id.to_string(),
            widget_type: widget_type.to_string(),
            window_id: "main_window".to_string(),
            window_index: 0,
            visible,
            enabled: true,
            x: 10,
            y: 20,
            width: 120,
            height: 24,
            text: None,
            value: None,
            checked: None,
            selected: None,
        }
    }

    fn orders_row(checked: bool) -> WidgetSnapshot {
        let mut row = snapshot("row:orders", "WamlProjectTreeRow", true);
        row.text = Some("Orders".to_string());
        row.value = Some("orders-diagram".to_string());
        row.checked = Some(checked);
        row.selected = Some("orders-diagram".to_string());
        row
    }

    fn surfaces(diagram_visible: bool, source_visible: bool) -> Vec<WidgetSnapshot> {
        vec![
            snapshot("canvas_wrap", "View", diagram_visible),
            snapshot("markdown_surface", "View", source_visible),
        ]
    }

    #[test]
    fn orders_is_active_when_its_row_is_checked_and_canvas_is_visible() {
        let mut widgets = vec![orders_row(true)];
        widgets.extend(surfaces(true, false));

        let observed = observe_active_diagram(&widgets, DiagramName::ORDERS).unwrap();

        assert_eq!(observed, "Orders diagram is active with a visible canvas");
    }

    #[test]
    fn ensure_open_uses_no_input_when_orders_is_already_active_in_diagram_view() {
        let mut widgets = vec![orders_row(true)];
        widgets.extend(surfaces(true, false));

        let decision = decide_diagram_open(&widgets, DiagramName::ORDERS).unwrap();

        assert_eq!(decision, DiagramOpenDecision::AlreadyActive);
    }

    #[test]
    fn unchecked_orders_row_is_not_an_active_diagram() {
        let mut widgets = vec![orders_row(false)];
        widgets.extend(surfaces(true, false));

        let error = observe_active_diagram(&widgets, DiagramName::ORDERS).unwrap_err();

        assert_eq!(error.observed, "Orders row is not checked");
    }

    #[test]
    fn checked_orders_row_without_a_visible_canvas_is_not_active() {
        let mut widgets = vec![orders_row(true)];
        widgets.extend(surfaces(false, true));

        let error = observe_active_diagram(&widgets, DiagramName::ORDERS).unwrap_err();

        assert_eq!(
            error.observed,
            "Orders row is checked but its canvas is hidden"
        );
    }

    #[test]
    fn checked_orders_row_with_two_visible_surfaces_is_not_active() {
        let mut widgets = vec![orders_row(true)];
        widgets.extend(surfaces(true, true));

        let error = observe_active_diagram(&widgets, DiagramName::ORDERS).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: Diagram and Source are visible"
        );
    }

    #[test]
    fn diagram_view_has_only_the_canvas_visible() {
        let observed = observe_active_view(&surfaces(true, false), ViewKind::Diagram).unwrap();

        assert_eq!(observed, "Diagram view is active");
    }

    #[test]
    fn source_view_has_only_the_markdown_surface_visible() {
        let observed = observe_active_view(&surfaces(false, true), ViewKind::Source).unwrap();

        assert_eq!(observed, "Source view is active");
    }

    #[test]
    fn missing_canvas_surface_is_invalid() {
        let widgets = vec![snapshot("markdown_surface", "View", true)];

        let error = observe_active_view(&widgets, ViewKind::Source).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: Diagram surface is missing"
        );
    }

    #[test]
    fn missing_source_surface_is_invalid() {
        let widgets = vec![snapshot("canvas_wrap", "View", true)];

        let error = observe_active_view(&widgets, ViewKind::Diagram).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: Source surface is missing"
        );
    }

    #[test]
    fn duplicate_canvas_surfaces_are_invalid() {
        let mut widgets = surfaces(true, false);
        widgets.push(snapshot("canvas_wrap", "View", false));

        let error = observe_active_view(&widgets, ViewKind::Diagram).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: 2 Diagram surfaces exist"
        );
    }

    #[test]
    fn duplicate_source_surfaces_are_invalid() {
        let mut widgets = surfaces(true, false);
        widgets.push(snapshot("markdown_surface", "View", false));

        let error = observe_active_view(&widgets, ViewKind::Diagram).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: 2 Source surfaces exist"
        );
    }

    #[test]
    fn switching_to_the_active_view_is_rejected_as_a_non_action() {
        let error =
            require_view_switch_is_a_change(&surfaces(false, true), ViewKind::Source).unwrap_err();

        assert_eq!(error.observed, "Source view is already active");
    }

    #[test]
    fn switching_to_the_other_valid_view_is_a_change() {
        require_view_switch_is_a_change(&surfaces(true, false), ViewKind::Source).unwrap();
    }

    #[test]
    fn two_visible_document_surfaces_are_invalid() {
        let error = observe_active_view(&surfaces(true, true), ViewKind::Diagram).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: Diagram and Source are visible"
        );
    }

    #[test]
    fn two_hidden_document_surfaces_are_invalid() {
        let error = observe_active_view(&surfaces(false, false), ViewKind::Source).unwrap_err();

        assert_eq!(
            error.observed,
            "invalid document surface state: Diagram and Source are hidden"
        );
    }
}
