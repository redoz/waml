use crate::source::SourceBundle;
use crate::{okf, uml};

pub type EditError = crate::ops::OpError;

#[derive(Clone, Copy)]
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf: &'a okf::Bundle,
    pub uml: &'a uml::Projection,
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
