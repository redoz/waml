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
        fs::write(
            self.artifacts_dir.join("semantic-trace.txt"),
            self.render_text(),
        )?;
        let json = serde_json::to_vec_pretty(&self.records)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(self.artifacts_dir.join("semantic-trace.json"), json)
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

#[cfg(test)]
mod tests {
    use super::SemanticTrace;
    use serde_json::Value;
    use std::fs;

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
}
