use crate::error::OperationFailure;
use crate::DiagramName;
use makepad_test::{Selector, TestApp, TestError, WidgetSnapshot};

pub(crate) fn expect_workspace_open(
    driver: &TestApp,
    diagram: DiagramName,
) -> Result<String, OperationFailure> {
    let locator = driver.locator(diagram_row_selector(diagram));
    locator
        .try_wait_visible()
        .map_err(|error| driver_failure(diagram, &error))?;
    let widgets = driver
        .try_widget_snapshot()
        .map_err(|error| driver_failure(diagram, &error))?;
    observe_workspace_open(&widgets, diagram)
}

fn observe_workspace_open(
    widgets: &[WidgetSnapshot],
    diagram: DiagramName,
) -> Result<String, OperationFailure> {
    let rows = diagram_rows(widgets, diagram);
    match rows.as_slice() {
        [] => Err(OperationFailure {
            observed: format!(
                "workspace has no visible enabled row for {}",
                diagram.display
            ),
            detail: format!(
                "expected one visible WAML diagram row with semantic value `{}`",
                diagram.value
            ),
        }),
        [row] if !row.enabled => Err(OperationFailure {
            observed: format!(
                "workspace has no visible enabled row for {}",
                diagram.display
            ),
            detail: format!("the {} row is visible but disabled", diagram.display),
        }),
        [row] if row.value.as_deref() != Some(diagram.value) => Err(OperationFailure {
            observed: format!("{} row has an invalid semantic value", diagram.display),
            detail: format!(
                "expected `{}`, found `{}`",
                diagram.value,
                row.value.as_deref().unwrap_or("<none>")
            ),
        }),
        [_] => Ok(format!(
            "workspace contains the available {} diagram",
            diagram.display
        )),
        rows => Err(OperationFailure {
            observed: format!(
                "workspace has {} visible enabled rows for {}",
                rows.len(),
                diagram.display
            ),
            detail: "the diagram row is ambiguous".to_string(),
        }),
    }
}

fn diagram_rows<'a>(
    widgets: &'a [WidgetSnapshot],
    diagram: DiagramName,
) -> Vec<&'a WidgetSnapshot> {
    widgets
        .iter()
        .filter(|widget| {
            widget.visible
                && widget.widget_type == "WamlProjectTreeRow"
                && widget.text.as_deref() == Some(diagram.display)
        })
        .collect()
}

fn diagram_row_selector(diagram: DiagramName) -> Selector {
    Selector::widget_type("WamlProjectTreeRow").text_exact(diagram.display)
}

fn driver_failure(diagram: DiagramName, error: &TestError) -> OperationFailure {
    OperationFailure {
        observed: format!(
            "workspace readiness for {} could not be observed",
            diagram.display
        ),
        detail: error.message().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::observe_workspace_open;
    use crate::DiagramName;
    use makepad_test::WidgetSnapshot;

    fn row(id: &str) -> WidgetSnapshot {
        WidgetSnapshot {
            id: id.to_string(),
            widget_type: "WamlProjectTreeRow".to_string(),
            window_id: "main_window".to_string(),
            window_index: 0,
            visible: true,
            enabled: true,
            x: 10,
            y: 20,
            width: 120,
            height: 24,
            text: Some("Orders".to_string()),
            value: Some("orders".to_string()),
            checked: Some(false),
            selected: Some("orders".to_string()),
        }
    }

    #[test]
    fn workspace_is_ready_when_orders_row_is_visible_and_enabled() {
        let observed = observe_workspace_open(&[row("row:orders")], DiagramName::ORDERS).unwrap();

        assert_eq!(observed, "workspace contains the available Orders diagram");
    }

    #[test]
    fn duplicate_orders_rows_are_an_ambiguous_workspace() {
        let error = observe_workspace_open(
            &[row("row:orders:first"), row("row:orders:second")],
            DiagramName::ORDERS,
        )
        .unwrap_err();

        assert_eq!(
            error.observed,
            "workspace has 2 visible enabled rows for Orders"
        );
    }

    #[test]
    fn disabled_orders_row_does_not_make_the_workspace_ready() {
        let mut disabled = row("row:orders");
        disabled.enabled = false;

        let error = observe_workspace_open(&[disabled], DiagramName::ORDERS).unwrap_err();

        assert_eq!(
            error.observed,
            "workspace has no visible enabled row for Orders"
        );
    }
}
