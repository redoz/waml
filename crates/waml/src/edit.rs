use crate::source::SourceBundle;
use crate::{analysis::OkfAnalysis, uml};
use std::fmt;

pub type EditError = crate::ops::OpError;

impl fmt::Display for crate::ops::OpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "edit step {} ({}) failed: {}",
            self.index, self.op, self.reason
        )
    }
}

impl std::error::Error for crate::ops::OpError {}

#[derive(Clone, Copy)]
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a OkfAnalysis,
    pub session_revision: u64,
    pub uml: &'a uml::Analysis,
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub trait EditBatch: sealed::Sealed {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError>;
}

pub struct PendingEdit(Box<dyn EditBatch>);

impl PendingEdit {
    pub fn new(batch: impl EditBatch + 'static) -> Self {
        Self(Box::new(batch))
    }
}

impl sealed::Sealed for PendingEdit {}

impl EditBatch for PendingEdit {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        self.0.lower(context)
    }
}
