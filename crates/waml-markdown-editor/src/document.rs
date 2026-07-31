use std::{fmt, sync::Arc};

use waml_syntax::{DocumentRevision, LineIndex, MarkdownSyntaxSnapshot, SourceText};

#[derive(Clone)]
pub struct MarkdownDocumentSnapshot {
    syntax: Arc<MarkdownSyntaxSnapshot>,
    line_index: Arc<LineIndex>,
}

impl MarkdownDocumentSnapshot {
    pub fn new(syntax: Arc<MarkdownSyntaxSnapshot>) -> Self {
        let line_index = Arc::new(LineIndex::new(syntax.text()));
        Self { syntax, line_index }
    }

    pub fn revision(&self) -> DocumentRevision {
        self.syntax.revision()
    }

    pub fn text(&self) -> &SourceText {
        self.syntax.text()
    }

    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    pub fn syntax(&self) -> &Arc<MarkdownSyntaxSnapshot> {
        &self.syntax
    }
}

impl fmt::Debug for MarkdownDocumentSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarkdownDocumentSnapshot")
            .field("revision", &self.revision())
            .field("source_len", &self.text().len())
            .finish()
    }
}
