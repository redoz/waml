use crate::config::WorkspaceBinding;
use crate::error::OperationFailure;
use crate::DiagramName;
use makepad_test::{Selector, TestApp, TestError, WidgetSnapshot};

pub(crate) fn expect_workspace_open(
    driver: &TestApp,
    workspace: WorkspaceBinding,
) -> Result<String, OperationFailure> {
    let diagram = workspace.ready_diagram;
    let locator = driver.locator(diagram_row_selector(diagram));
    locator
        .try_wait_visible()
        .map_err(|error| driver_failure(diagram, &error))?;
    let initial = driver
        .try_widget_snapshot()
        .map_err(|error| driver_failure(diagram, &error))?;
    if decide_workspace_readiness(&initial, workspace)? == WorkspaceReadiness::WaitForEnabled {
        locator
            .try_wait_enabled(true)
            .map_err(|error| readiness_wait_failure(driver, workspace, &error))?;
    }
    let widgets = driver
        .try_widget_snapshot()
        .map_err(|error| driver_failure(diagram, &error))?;
    observe_workspace_open(&widgets, workspace)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceReadiness {
    Ready,
    WaitForEnabled,
}

fn decide_workspace_readiness(
    widgets: &[WidgetSnapshot],
    workspace: WorkspaceBinding,
) -> Result<WorkspaceReadiness, OperationFailure> {
    resolve_workspace_root(widgets, workspace)?;
    let row = resolve_workspace_row(widgets, workspace.ready_diagram)?;
    if row.enabled {
        Ok(WorkspaceReadiness::Ready)
    } else {
        Ok(WorkspaceReadiness::WaitForEnabled)
    }
}

fn observe_workspace_open(
    widgets: &[WidgetSnapshot],
    workspace: WorkspaceBinding,
) -> Result<String, OperationFailure> {
    resolve_workspace_root(widgets, workspace)?;
    let diagram = workspace.ready_diagram;
    let row = resolve_workspace_row(widgets, diagram)?;
    if !row.enabled {
        return Err(OperationFailure {
            observed: format!(
                "workspace has no visible enabled row for {}",
                diagram.display
            ),
            detail: format!("the {} row is visible but disabled", diagram.display),
        });
    }
    Ok(format!(
        "{} workspace contains the available {} diagram",
        workspace.root.title, diagram.display
    ))
}

fn resolve_workspace_root(
    widgets: &[WidgetSnapshot],
    workspace: WorkspaceBinding,
) -> Result<&WidgetSnapshot, OperationFailure> {
    let roots: Vec<_> = widgets
        .iter()
        .filter(|widget| {
            widget.visible
                && widget.widget_type == "WamlProjectTreeRow"
                && widget.value.as_deref() == Some(workspace.root.value)
        })
        .collect();
    let root = match roots.as_slice() {
        [] => {
            return Err(OperationFailure {
                observed: "workspace has no visible semantic root".to_string(),
                detail: format!(
                    "expected one visible WamlProjectTreeRow with title `{}` and semantic value `{}`",
                    workspace.root.title, workspace.root.value
                ),
            });
        }
        [root] => *root,
        roots => {
            return Err(OperationFailure {
                observed: format!("workspace has {} visible semantic roots", roots.len()),
                detail: "the workspace root is ambiguous".to_string(),
            });
        }
    };
    if root.text.as_deref() != Some(workspace.root.title) {
        return Err(OperationFailure {
            observed: format!(
                "workspace root is {}, not {}",
                root.text.as_deref().unwrap_or("<none>"),
                workspace.root.title
            ),
            detail: format!(
                "expected root title `{}` with semantic value `{}`",
                workspace.root.title, workspace.root.value
            ),
        });
    }
    if root.enabled {
        return Err(OperationFailure {
            observed: format!("{} workspace root is actionable", workspace.root.title),
            detail: "the semantic root row must be disabled".to_string(),
        });
    }
    Ok(root)
}

fn resolve_workspace_row(
    widgets: &[WidgetSnapshot],
    diagram: DiagramName,
) -> Result<&WidgetSnapshot, OperationFailure> {
    let rows = diagram_rows(widgets, diagram);
    let row = match rows.as_slice() {
        [] => {
            return Err(OperationFailure {
                observed: format!(
                    "workspace has no visible enabled row for {}",
                    diagram.display
                ),
                detail: format!(
                    "expected one visible WAML diagram row with semantic value `{}`",
                    diagram.value
                ),
            });
        }
        [row] => *row,
        rows => {
            return Err(OperationFailure {
                observed: format!(
                    "workspace has {} visible enabled rows for {}",
                    rows.len(),
                    diagram.display
                ),
                detail: "the diagram row is ambiguous".to_string(),
            });
        }
    };
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

fn diagram_rows(widgets: &[WidgetSnapshot], diagram: DiagramName) -> Vec<&WidgetSnapshot> {
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

fn readiness_wait_failure(
    driver: &TestApp,
    workspace: WorkspaceBinding,
    error: &TestError,
) -> OperationFailure {
    let diagram = workspace.ready_diagram;
    let Some(mut failure) = driver
        .try_widget_snapshot()
        .ok()
        .and_then(|widgets| observe_workspace_open(&widgets, workspace).err())
    else {
        return driver_failure(diagram, error);
    };
    failure.detail = format!("{}; {}", failure.detail, error.message());
    failure
}

#[cfg(test)]
mod tests {
    use super::{decide_workspace_readiness, observe_workspace_open, WorkspaceReadiness};
    use crate::config::{WorkspaceBinding, WorkspaceRootFingerprint};
    use crate::DiagramName;
    use makepad_test::WidgetSnapshot;

    const MINI: WorkspaceBinding = WorkspaceBinding {
        root: WorkspaceRootFingerprint {
            title: "Mini",
            value: "/",
        },
        ready_diagram: DiagramName::ORDERS,
    };

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
            value: Some("orders-diagram".to_string()),
            checked: Some(false),
            selected: Some("orders-diagram".to_string()),
        }
    }

    fn root(title: &str) -> WidgetSnapshot {
        WidgetSnapshot {
            id: format!("project-tree-row:root:{title}"),
            widget_type: "WamlProjectTreeRow".to_string(),
            window_id: "main_window".to_string(),
            window_index: 0,
            visible: true,
            enabled: false,
            x: 10,
            y: 0,
            width: 120,
            height: 24,
            text: Some(title.to_string()),
            value: Some("/".to_string()),
            checked: Some(false),
            selected: None,
        }
    }

    #[test]
    fn workspace_is_ready_when_the_bound_root_and_diagram_are_exact() {
        let observed = observe_workspace_open(&[root("Mini"), row("row:orders")], MINI).unwrap();

        assert_eq!(
            observed,
            "Mini workspace contains the available Orders diagram"
        );
    }

    #[test]
    fn another_orders_workspace_does_not_satisfy_the_mini_binding() {
        let error =
            observe_workspace_open(&[root("Mixed OKF"), row("row:orders")], MINI).unwrap_err();

        assert_eq!(error.observed, "workspace root is Mixed OKF, not Mini");
        assert!(error.detail.contains("expected root title `Mini`"));
        assert!(error.detail.contains("semantic value `/`"));
    }

    #[test]
    fn duplicate_semantic_roots_are_an_ambiguous_workspace() {
        let error =
            observe_workspace_open(&[root("Mini"), root("Mixed OKF"), row("row:orders")], MINI)
                .unwrap_err();

        assert_eq!(error.observed, "workspace has 2 visible semantic roots");
        assert_eq!(error.detail, "the workspace root is ambiguous");
    }

    #[test]
    fn actionable_semantic_root_does_not_satisfy_the_workspace_contract() {
        let mut actionable = root("Mini");
        actionable.enabled = true;

        let error = observe_workspace_open(&[actionable, row("row:orders")], MINI).unwrap_err();

        assert_eq!(error.observed, "Mini workspace root is actionable");
        assert_eq!(error.detail, "the semantic root row must be disabled");
    }

    #[test]
    fn root_with_another_semantic_value_does_not_satisfy_the_binding() {
        let mut wrong_value = root("Mini");
        wrong_value.value = Some("/other".to_string());

        let error = observe_workspace_open(&[wrong_value, row("row:orders")], MINI).unwrap_err();

        assert_eq!(error.observed, "workspace has no visible semantic root");
        assert!(error.detail.contains("semantic value `/`"));
    }

    #[test]
    fn invisible_root_does_not_satisfy_the_workspace_binding() {
        let mut invisible = root("Mini");
        invisible.visible = false;

        let error = observe_workspace_open(&[invisible, row("row:orders")], MINI).unwrap_err();

        assert_eq!(error.observed, "workspace has no visible semantic root");
    }

    #[test]
    fn another_widget_type_does_not_satisfy_the_workspace_binding() {
        let mut wrong_type = root("Mini");
        wrong_type.widget_type = "Label".to_string();

        let error = observe_workspace_open(&[wrong_type, row("row:orders")], MINI).unwrap_err();

        assert_eq!(error.observed, "workspace has no visible semantic root");
    }

    #[test]
    fn duplicate_orders_rows_are_an_ambiguous_workspace() {
        let error = observe_workspace_open(
            &[
                root("Mini"),
                row("row:orders:first"),
                row("row:orders:second"),
            ],
            MINI,
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

        let error = observe_workspace_open(&[root("Mini"), disabled], MINI).unwrap_err();

        assert_eq!(
            error.observed,
            "workspace has no visible enabled row for Orders"
        );
    }

    #[test]
    fn visible_disabled_orders_row_requires_an_enabled_readiness_wait() {
        let mut disabled = row("row:orders");
        disabled.enabled = false;

        let decision = decide_workspace_readiness(&[root("Mini"), disabled], MINI).unwrap();

        assert_eq!(decision, WorkspaceReadiness::WaitForEnabled);
    }
}
