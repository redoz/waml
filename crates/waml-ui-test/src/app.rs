use crate::adapters::{documents, rendering, search, tree, workspace};
use crate::config::WorkspaceBinding;
use crate::domain::{DiagramName, ViewKind};
use crate::error::{OperationFailure, WamlUiError};
use crate::trace::SemanticTrace;
use std::path::{Path, PathBuf};

pub struct WamlApp {
    driver: makepad_test::TestApp,
    test_name: String,
    artifacts_dir: PathBuf,
    references_dir: PathBuf,
    recordings_dir: PathBuf,
    workspace: WorkspaceBinding,
    trace: SemanticTrace,
}

impl WamlApp {
    pub(crate) fn new(
        driver: makepad_test::TestApp,
        test_name: String,
        artifacts_dir: PathBuf,
        references_dir: PathBuf,
        recordings_dir: PathBuf,
        workspace: WorkspaceBinding,
        trace: SemanticTrace,
    ) -> Self {
        Self {
            driver,
            test_name,
            artifacts_dir,
            references_dir,
            recordings_dir,
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

    /// Assert the project tree's currently-drawn rows, by title, top to
    /// bottom. Rows scrolled out of the panel viewport are not part of the
    /// list. The operation also holds the tree to its layout invariant --
    /// every listed row occupies a non-zero rect, and the rows run down the
    /// panel without overlapping -- so a row that stays in the model while
    /// it stops being drawn fails here rather than reading as present.
    pub fn expect_project_tree_rows(&mut self, rows: &[&str]) -> &mut Self {
        let rows = rows.to_vec();
        self.execute(
            format!("expect project tree rows {}", describe_titles(&rows)),
            format!("the tree shows {}", describe_titles(&rows)),
            move |driver| tree::expect_project_tree_rows(driver, &rows),
        )
    }

    /// Assert exactly one project-tree row is selected, that it is `row`, and
    /// that it is inside the panel viewport -- a selection scrolled out of
    /// view fails, which is the "reveal landed on the right row" claim.
    pub fn expect_selected_row(&mut self, row: &str) -> &mut Self {
        let row = row.to_string();
        self.execute(
            format!("expect selected row {row}"),
            format!("the {row} row is selected and in view"),
            move |driver| tree::expect_selected_row(driver, &row),
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

    /// Ctrl+K (Cmd+K on macOS): open the search palette.
    pub fn open_search_palette(&mut self) -> &mut Self {
        self.execute(
            "open search palette",
            "the search palette is open",
            search::open_search_palette,
        )
    }

    /// Type `query` into whichever search surface currently owns keyboard
    /// input (the palette or the find strip).
    pub fn type_search_query(&mut self, query: &str) -> &mut Self {
        let query = query.to_string();
        self.execute(
            format!("type search query \"{query}\""),
            format!("the query is now \"{query}\""),
            move |driver| search::type_search_query(driver, &query),
        )
    }

    /// Assert the palette's currently-rendered titled sections, as
    /// `(title, row count)` pairs in the blended list's own order (CONCEPTS,
    /// DOCUMENTS, TEXT, STRUCTURE, RECENT).
    pub fn expect_palette_sections(&mut self, sections: &[(&str, usize)]) -> &mut Self {
        let sections = sections.to_vec();
        self.execute(
            format!("expect palette sections {}", describe_pairs(&sections)),
            format!("palette sections are {}", describe_pairs(&sections)),
            move |driver| search::expect_palette_sections(driver, &sections),
        )
    }

    /// Commit the palette's trailing escalate row, opening the full results
    /// tab for the current query.
    pub fn escalate_to_results_tab(&mut self) -> &mut Self {
        self.execute(
            "escalate to results tab",
            "the results tab is active",
            search::escalate_to_results_tab,
        )
    }

    /// Assert the results tab's document groups, as `(document path, row
    /// count)` pairs in rank order.
    pub fn expect_results_grouped_by_document(&mut self, groups: &[(&str, usize)]) -> &mut Self {
        let groups = groups.to_vec();
        self.execute(
            format!(
                "expect results grouped by document {}",
                describe_pairs(&groups)
            ),
            format!("results are grouped as {}", describe_pairs(&groups)),
            move |driver| search::expect_results_grouped_by_document(driver, &groups),
        )
    }

    /// Ctrl+F (Cmd+F on macOS): open the find-in-document strip.
    pub fn open_find_strip(&mut self) -> &mut Self {
        self.execute(
            "open find strip",
            "the find strip is open",
            search::open_find_strip,
        )
    }

    /// F3: step the open find session to the next hit, wrapping past the
    /// last one back to the first.
    pub fn advance_to_next_hit(&mut self) -> &mut Self {
        self.execute(
            "advance to next hit",
            "the find cursor stepped to the next hit",
            |driver| search::advance_find_hit(driver, true),
        )
    }

    /// Shift+F3: step the open find session to the previous hit, wrapping
    /// past the first one back to the last.
    pub fn advance_to_previous_hit(&mut self) -> &mut Self {
        self.execute(
            "advance to previous hit",
            "the find cursor stepped to the previous hit",
            |driver| search::advance_find_hit(driver, false),
        )
    }

    /// Assert the find strip's `"{n} of {total}"` counter reading.
    pub fn expect_find_counter(&mut self, text: &str) -> &mut Self {
        let text = text.to_string();
        self.execute(
            format!("expect find counter \"{text}\""),
            format!("the find counter reads \"{text}\""),
            move |driver| search::expect_find_counter(driver, &text),
        )
    }

    /// The rendering gate: hold the diagram canvas to the reference stored for
    /// this platform.
    ///
    /// This is the ONE operation in this crate that is about pixels, and it is
    /// deliberately narrow. It compares INK -- whether a pixel is background
    /// or not -- over the canvas rect only, so it settles where connectors
    /// run, how thick a stroke quantises, and where glyphs sit, and settles
    /// nothing about colour or antialias quality. `crate::reference` carries
    /// the argument for that choice; `adapters::rendering` carries the rule
    /// for when a mismatch fails rather than records.
    ///
    /// Re-record an intended change with
    /// `WAML_UI_TEST_UPDATE_REFERENCES=1` and commit the reference.
    pub fn expect_canvas_matches_reference(&mut self, name: &str) -> &mut Self {
        let name = name.to_string();
        let references_dir = self.references_dir.clone();
        let recordings_dir = self.recordings_dir.clone();
        let artifacts_dir = self.artifacts_dir.clone();
        self.execute(
            format!("expect canvas matches reference {name}"),
            format!("the {name} canvas is drawn the way its stored reference was"),
            move |driver| {
                rendering::expect_canvas_matches_reference(
                    driver,
                    &references_dir,
                    &recordings_dir,
                    &artifacts_dir,
                    &name,
                )
            },
        )
    }
}

fn describe_titles(titles: &[&str]) -> String {
    if titles.is_empty() {
        return "no rows".to_string();
    }
    titles.join(", ")
}

fn describe_pairs(pairs: &[(&str, usize)]) -> String {
    if pairs.is_empty() {
        return "<none>".to_string();
    }
    pairs
        .iter()
        .map(|(label, count)| format!("{label} \u{00B7} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
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
