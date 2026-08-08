use serde::Serialize;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize)]
pub struct StepRecord {
    pub sequence: u32,
    pub operation: String,
    pub expected: String,
    pub observed: String,
    pub outcome: StepOutcome,
}

#[derive(Clone, Debug, Serialize)]
pub enum StepOutcome {
    Running,
    Passed,
    Failed { detail: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistPhase {
    PrepareText,
    PrepareJson,
    PublishText,
    PublishJson,
}

struct TracePaths {
    text: PathBuf,
    json: PathBuf,
    next_text: PathBuf,
    next_json: PathBuf,
    previous_text: PathBuf,
    previous_json: PathBuf,
}

#[derive(Default)]
struct PublishState {
    text_backed_up: bool,
    json_backed_up: bool,
    text_published: bool,
    json_published: bool,
}

pub(crate) struct SemanticTrace {
    artifacts_dir: PathBuf,
    records: Vec<StepRecord>,
}

impl SemanticTrace {
    pub(crate) fn new(artifacts_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let artifacts_dir = artifacts_dir.into();
        fs::create_dir_all(&artifacts_dir)?;
        Ok(Self {
            artifacts_dir,
            records: Vec::new(),
        })
    }

    pub(crate) fn begin(
        &mut self,
        operation: impl Into<String>,
        expected: impl Into<String>,
    ) -> io::Result<u32> {
        let sequence = u32::try_from(self.records.len() + 1).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "semantic trace is too long")
        })?;
        self.records.push(StepRecord {
            sequence,
            operation: operation.into(),
            expected: expected.into(),
            observed: "not observed yet".to_string(),
            outcome: StepOutcome::Running,
        });
        self.persist()?;
        Ok(sequence)
    }

    pub(crate) fn pass(&mut self, sequence: u32, observed: impl Into<String>) -> io::Result<()> {
        let record = self.running_record_mut(sequence)?;
        record.observed = observed.into();
        record.outcome = StepOutcome::Passed;
        self.persist()
    }

    pub(crate) fn fail(
        &mut self,
        sequence: u32,
        observed: impl Into<String>,
        detail: impl Into<String>,
    ) -> io::Result<()> {
        let record = self.running_record_mut(sequence)?;
        record.observed = observed.into();
        record.outcome = StepOutcome::Failed {
            detail: detail.into(),
        };
        self.persist()
    }

    fn running_record_mut(&mut self, sequence: u32) -> io::Result<&mut StepRecord> {
        let record = self
            .records
            .iter_mut()
            .find(|record| record.sequence == sequence)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("semantic trace has no step {sequence}"),
                )
            })?;
        if !matches!(record.outcome, StepOutcome::Running) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("semantic trace step {sequence} is already complete"),
            ));
        }
        Ok(record)
    }

    fn persist(&self) -> io::Result<()> {
        self.persist_with_hook(|_| Ok(()))
    }

    fn persist_with_hook(
        &self,
        mut hook: impl FnMut(PersistPhase) -> io::Result<()>,
    ) -> io::Result<()> {
        let text = self.render_text();
        let json = serde_json::to_vec_pretty(&self.records)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let paths = TracePaths::new(&self.artifacts_dir);
        let mut state = PublishState::default();
        let transaction = (|| {
            remove_file_if_present(&paths.next_text)?;
            remove_file_if_present(&paths.next_json)?;
            remove_file_if_present(&paths.previous_text)?;
            remove_file_if_present(&paths.previous_json)?;

            hook(PersistPhase::PrepareText)?;
            fs::write(&paths.next_text, text)?;
            hook(PersistPhase::PrepareJson)?;
            fs::write(&paths.next_json, json)?;

            let text_exists = paths.text.try_exists()?;
            let json_exists = paths.json.try_exists()?;
            if text_exists != json_exists {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "semantic trace text and JSON files are not a complete pair",
                ));
            }
            if text_exists {
                fs::rename(&paths.text, &paths.previous_text)?;
                state.text_backed_up = true;
                fs::rename(&paths.json, &paths.previous_json)?;
                state.json_backed_up = true;
            }

            hook(PersistPhase::PublishText)?;
            fs::rename(&paths.next_text, &paths.text)?;
            state.text_published = true;
            hook(PersistPhase::PublishJson)?;
            fs::rename(&paths.next_json, &paths.json)?;
            state.json_published = true;
            Ok(())
        })();

        if let Err(error) = transaction {
            let rollback = rollback_publish(&paths, &state);
            return Err(match rollback {
                Ok(()) => error,
                Err(rollback_error) => io::Error::new(
                    error.kind(),
                    format!("{error}; trace rollback also failed: {rollback_error}"),
                ),
            });
        }

        let _ = remove_file_if_present(&paths.previous_text);
        let _ = remove_file_if_present(&paths.previous_json);
        Ok(())
    }

    fn render_text(&self) -> String {
        let mut text = String::new();
        for record in &self.records {
            let (outcome, detail) = match &record.outcome {
                StepOutcome::Running => ("running", None),
                StepOutcome::Passed => ("passed", None),
                StepOutcome::Failed { detail } => ("failed", Some(detail.as_str())),
            };
            let _ = writeln!(
                text,
                "Step {}: {} [{}]",
                record.sequence, record.operation, outcome
            );
            let _ = writeln!(text, "Expected: {}", record.expected);
            let _ = writeln!(text, "Observed: {}", record.observed);
            if let Some(detail) = detail {
                let _ = writeln!(text, "Detail: {detail}");
            }
            text.push('\n');
        }
        text
    }
}

impl TracePaths {
    fn new(artifacts_dir: &std::path::Path) -> Self {
        Self {
            text: artifacts_dir.join("semantic-trace.txt"),
            json: artifacts_dir.join("semantic-trace.json"),
            next_text: artifacts_dir.join("semantic-trace.txt.next"),
            next_json: artifacts_dir.join("semantic-trace.json.next"),
            previous_text: artifacts_dir.join("semantic-trace.txt.previous"),
            previous_json: artifacts_dir.join("semantic-trace.json.previous"),
        }
    }
}

fn rollback_publish(paths: &TracePaths, state: &PublishState) -> io::Result<()> {
    let mut failures = Vec::new();
    if state.text_published {
        collect_file_result(&mut failures, remove_file_if_present(&paths.text));
    }
    if state.json_published {
        collect_file_result(&mut failures, remove_file_if_present(&paths.json));
    }
    if state.text_backed_up {
        collect_file_result(&mut failures, fs::rename(&paths.previous_text, &paths.text));
    }
    if state.json_backed_up {
        collect_file_result(&mut failures, fs::rename(&paths.previous_json, &paths.json));
    }
    collect_file_result(&mut failures, remove_file_if_present(&paths.next_text));
    collect_file_result(&mut failures, remove_file_if_present(&paths.next_json));

    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(failures.join("; ")))
    }
}

fn collect_file_result(failures: &mut Vec<String>, result: io::Result<()>) {
    if let Err(error) = result {
        failures.push(error.to_string());
    }
}

fn remove_file_if_present(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{PersistPhase, SemanticTrace, StepOutcome};
    use serde_json::Value;
    use std::fs;
    use std::io;

    #[test]
    fn begin_immediately_persists_a_running_record_in_text_and_json() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();

        let sequence = trace
            .begin("open diagram Orders", "Orders is the active diagram")
            .unwrap();

        assert_eq!(sequence, 1);
        let text = fs::read_to_string(temp.path().join("semantic-trace.txt")).unwrap();
        assert!(text.contains("Step 1: open diagram Orders [running]"));
        assert!(text.contains("Expected: Orders is the active diagram"));
        let json: Value =
            serde_json::from_slice(&fs::read(temp.path().join("semantic-trace.json")).unwrap())
                .unwrap();
        assert_eq!(json[0]["sequence"], 1);
        assert_eq!(json[0]["operation"], "open diagram Orders");
        assert_eq!(json[0]["outcome"], "Running");
    }

    #[test]
    fn pass_replaces_the_running_outcome_and_persists_observed_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();
        let sequence = trace.begin("open Orders", "Orders is active").unwrap();

        trace.pass(sequence, "Orders is active").unwrap();

        let text = fs::read_to_string(temp.path().join("semantic-trace.txt")).unwrap();
        assert!(text.contains("Step 1: open Orders [passed]"));
        assert!(text.contains("Observed: Orders is active"));
        let json: Value =
            serde_json::from_slice(&fs::read(temp.path().join("semantic-trace.json")).unwrap())
                .unwrap();
        assert_eq!(json[0]["outcome"], "Passed");
        assert_eq!(json[0]["observed"], "Orders is active");
    }

    #[test]
    fn fail_replaces_the_running_outcome_and_persists_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();
        let sequence = trace.begin("switch to Source", "Source is active").unwrap();

        trace
            .fail(
                sequence,
                "Diagram remained active",
                "source control was disabled",
            )
            .unwrap();

        let text = fs::read_to_string(temp.path().join("semantic-trace.txt")).unwrap();
        assert!(text.contains("Step 1: switch to Source [failed]"));
        assert!(text.contains("Observed: Diagram remained active"));
        assert!(text.contains("Detail: source control was disabled"));
        let json: Value =
            serde_json::from_slice(&fs::read(temp.path().join("semantic-trace.json")).unwrap())
                .unwrap();
        assert_eq!(
            json[0]["outcome"]["Failed"]["detail"],
            "source control was disabled"
        );
    }

    #[test]
    fn json_preparation_failure_preserves_the_previous_trace_pair() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();
        trace.begin("open Orders", "Orders is active").unwrap();
        let text_path = temp.path().join("semantic-trace.txt");
        let json_path = temp.path().join("semantic-trace.json");
        let previous_text = fs::read(&text_path).unwrap();
        let previous_json = fs::read(&json_path).unwrap();
        trace.records[0].observed = "Orders is active".to_string();
        trace.records[0].outcome = StepOutcome::Passed;

        let error = trace
            .persist_with_hook(|phase| {
                if phase == PersistPhase::PrepareJson {
                    Err(io::Error::other("injected JSON preparation failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("injected JSON preparation failure"));
        assert_eq!(fs::read(text_path).unwrap(), previous_text);
        assert_eq!(fs::read(json_path).unwrap(), previous_json);
    }

    #[test]
    fn json_publish_failure_rolls_back_to_the_previous_trace_pair() {
        let temp = tempfile::tempdir().unwrap();
        let mut trace = SemanticTrace::new(temp.path()).unwrap();
        trace.begin("open Orders", "Orders is active").unwrap();
        let text_path = temp.path().join("semantic-trace.txt");
        let json_path = temp.path().join("semantic-trace.json");
        let previous_text = fs::read(&text_path).unwrap();
        let previous_json = fs::read(&json_path).unwrap();
        trace.records[0].observed = "Orders is active".to_string();
        trace.records[0].outcome = StepOutcome::Passed;

        let error = trace
            .persist_with_hook(|phase| {
                if phase == PersistPhase::PublishJson {
                    Err(io::Error::other("injected JSON publish failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();

        assert!(error.to_string().contains("injected JSON publish failure"));
        assert_eq!(fs::read(text_path).unwrap(), previous_text);
        assert_eq!(fs::read(json_path).unwrap(), previous_json);
    }
}
