use crate::adapters::{documents, workspace};
use crate::config::WorkspaceBinding;
use crate::domain::{DiagramName, ViewKind};
use crate::error::{OperationFailure, WamlUiError};
use crate::trace::SemanticTrace;
use std::path::{Path, PathBuf};

pub struct WamlApp {
    driver: makepad_test::TestApp,
    test_name: String,
    artifacts_dir: PathBuf,
    workspace: WorkspaceBinding,
    trace: SemanticTrace,
}

impl WamlApp {
    pub(crate) fn new(
        driver: makepad_test::TestApp,
        test_name: String,
        artifacts_dir: PathBuf,
        workspace: WorkspaceBinding,
        trace: SemanticTrace,
    ) -> Self {
        Self {
            driver,
            test_name,
            artifacts_dir,
            workspace,
            trace,
        }
    }

    pub(crate) fn execute(
        &mut self,
        operation: impl Into<String>,
        expected: impl Into<String>,
        action: impl FnOnce(&makepad_test::TestApp) -> Result<String, OperationFailure>,
    ) -> &mut Self {
        let driver = &self.driver;
        execute_envelope(
            &mut self.trace,
            &self.test_name,
            &self.artifacts_dir,
            operation,
            expected,
            || action(driver),
        );
        self
    }

    pub fn expect_workspace_open(&mut self) -> &mut Self {
        let workspace = self.workspace;
        self.execute(
            "expect workspace open",
            format!(
                "{} workspace contains the available {} diagram",
                workspace.root.title, workspace.ready_diagram.display
            ),
            |driver| workspace::expect_workspace_open(driver, workspace),
        )
    }

    pub fn ensure_diagram_open(&mut self, diagram: DiagramName) -> &mut Self {
        self.execute(
            format!("ensure {} diagram open", diagram.display),
            format!("{} diagram is active in Diagram view", diagram.display),
            |driver| documents::ensure_diagram_open(driver, diagram),
        )
    }

    pub fn expect_active_diagram(&mut self, diagram: DiagramName) -> &mut Self {
        self.execute(
            format!("expect active diagram {}", diagram.display),
            format!(
                "{} diagram is active with a visible canvas",
                diagram.display
            ),
            |driver| documents::expect_active_diagram(driver, diagram),
        )
    }

    pub fn switch_active_document_to(&mut self, view: ViewKind) -> &mut Self {
        self.execute(
            format!("switch active document to {}", view_name(view)),
            format!("{} view is active", view_name(view)),
            |driver| documents::switch_active_document_to(driver, view),
        )
    }

    pub fn expect_active_view(&mut self, view: ViewKind) -> &mut Self {
        self.execute(
            format!("expect active view {}", view_name(view)),
            format!("{} view is active", view_name(view)),
            |driver| documents::expect_active_view(driver, view),
        )
    }
}

fn view_name(view: ViewKind) -> &'static str {
    match view {
        ViewKind::Diagram => "Diagram",
        ViewKind::Source => "Source",
    }
}

fn execute_envelope(
    trace: &mut SemanticTrace,
    test_name: &str,
    artifacts_dir: &Path,
    operation: impl Into<String>,
    expected: impl Into<String>,
    action: impl FnOnce() -> Result<String, OperationFailure>,
) {
    let operation = operation.into();
    let expected = expected.into();
    let sequence = trace
        .begin(operation.clone(), expected.clone())
        .unwrap_or_else(|error| {
            panic!(
                "failed to persist running semantic step for `{operation}`: {error}; artifacts: {}",
                artifacts_dir.display()
            )
        });
    match action() {
        Ok(observed) => {
            if let Err(error) = trace.pass(sequence, observed.clone()) {
                panic!(
                    "{}",
                    WamlUiError {
                        test_name: test_name.to_string(),
                        sequence,
                        operation,
                        expected,
                        observed,
                        detail: format!("failed to persist passed semantic trace: {error}"),
                        artifacts_dir: artifacts_dir.to_path_buf(),
                    }
                );
            }
        }
        Err(failure) => {
            let mut detail = failure.detail;
            if let Err(error) = trace.fail(sequence, failure.observed.clone(), detail.clone()) {
                detail.push_str(&format!(
                    "; failed to persist failed semantic trace: {error}"
                ));
            }
            panic!(
                "{}",
                WamlUiError {
                    test_name: test_name.to_string(),
                    sequence,
                    operation,
                    expected,
                    observed: failure.observed,
                    detail,
                    artifacts_dir: artifacts_dir.to_path_buf(),
                }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::execute_envelope;
    use crate::error::OperationFailure;
    use crate::trace::SemanticTrace;
    use serde_json::Value;
    use std::fs;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn execute_envelope_persists_running_before_multiple_interactions() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();
        let trace_path = temp.path().join("semantic-trace.json");
        let mut interactions = Vec::new();

        execute_envelope(
            &mut trace,
            "ui::opens_orders",
            temp.path(),
            "open Orders",
            "Orders is active",
            || {
                let json: Value = serde_json::from_slice(&fs::read(&trace_path).unwrap()).unwrap();
                assert_eq!(json[0]["outcome"], "Running");
                interactions.push("locate");
                interactions.push("click");
                Ok("Orders is active".to_string())
            },
        );

        assert_eq!(interactions, ["locate", "click"]);
        let json: Value = serde_json::from_slice(&fs::read(trace_path).unwrap()).unwrap();
        assert_eq!(json[0]["outcome"], "Passed");
        assert_eq!(json[0]["observed"], "Orders is active");
    }

    #[test]
    fn execute_envelope_records_a_zero_interaction_failure_before_panicking() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();

        let panic = catch_unwind(AssertUnwindSafe(|| {
            execute_envelope(
                &mut trace,
                "ui::switch_view",
                temp.path(),
                "switch to Source",
                "Source is active",
                || {
                    Err(OperationFailure {
                        observed: "Diagram remained active".to_string(),
                        detail: "source control was disabled".to_string(),
                    })
                },
            );
        }))
        .unwrap_err();

        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap();
        assert!(message.contains("Step 1: switch to Source failed"));
        assert!(message.contains("Observed: Diagram remained active"));
        assert!(message.contains("Detail: source control was disabled"));
        let json: Value =
            serde_json::from_slice(&fs::read(temp.path().join("semantic-trace.json")).unwrap())
                .unwrap();
        assert_eq!(
            json[0]["outcome"]["Failed"]["detail"],
            "source control was disabled"
        );
    }
}
