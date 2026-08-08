use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct OperationFailure {
    pub observed: String,
    pub detail: String,
}

#[derive(Debug)]
pub struct WamlUiError {
    pub test_name: String,
    pub sequence: u32,
    pub operation: String,
    pub expected: String,
    pub observed: String,
    pub detail: String,
    pub artifacts_dir: PathBuf,
}

impl fmt::Display for WamlUiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Step {}: {} failed\n\
             Test: {}\n\
             Expected: {}\n\
             Observed: {}\n\
             Detail: {}\n\
             Artifacts: {}",
            self.sequence,
            self.operation,
            self.test_name,
            self.expected,
            self.observed,
            self.detail,
            self.artifacts_dir.display()
        )
    }
}

impl std::error::Error for WamlUiError {}

#[cfg(test)]
mod tests {
    use super::WamlUiError;
    use std::path::PathBuf;

    #[test]
    fn display_reports_semantic_context_and_low_level_evidence() {
        let error = WamlUiError {
            test_name: "open_and_switch_document_views".to_string(),
            sequence: 4,
            operation: "switch active document to Source".to_string(),
            expected: "active view is Source for Orders".to_string(),
            observed: "active view remained Diagram".to_string(),
            detail: "source control was disabled".to_string(),
            artifacts_dir: PathBuf::from(
                "target/waml-ui-test/123-1/open-and-switch-document-views",
            ),
        };

        assert_eq!(
            error.to_string(),
            "Step 4: switch active document to Source failed\n\
             Test: open_and_switch_document_views\n\
             Expected: active view is Source for Orders\n\
             Observed: active view remained Diagram\n\
             Detail: source control was disabled\n\
             Artifacts: target/waml-ui-test/123-1/open-and-switch-document-views"
        );
    }
}
