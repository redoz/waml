use std::{collections::BTreeSet, fmt, sync::Arc};

use waml_syntax::{
    RewriteError, SyntaxElement, SyntaxLanguage, SyntaxLocator, SyntaxNode, SyntaxToken,
    SyntaxTree, TextRange, TextSize,
};

use crate::{
    analysis::DocumentCatalog,
    edit::{EditBatch, EditContext, EditError},
    source::{DocumentId, SourceBundle},
};
use waml_syntax::DocumentRevision;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    pub range: TextRange,
    pub replacement: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedDocumentChange {
    pub document: DocumentId,
    pub base_document_revision: DocumentRevision,
    pub edits: Arc<[TextEdit]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionBasis {
    Document {
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
    },
    Bundle {
        session_revision: u64,
    },
}

impl ActionBasis {
    fn session_revision(&self) -> u64 {
        match self {
            Self::Document {
                session_revision, ..
            }
            | Self::Bundle { session_revision } => *session_revision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeAction {
    pub title: String,
    pub basis: ActionBasis,
    pub changes: Arc<[VersionedDocumentChange]>,
}

#[derive(Clone, Debug)]
pub struct VersionedSyntaxLocator<L: SyntaxLanguage> {
    document: DocumentId,
    document_revision: DocumentRevision,
    session_revision: u64,
    locator: SyntaxLocator<L>,
}

impl<L: SyntaxLanguage> VersionedSyntaxLocator<L> {
    pub fn for_node(
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
        node: &SyntaxNode<L>,
    ) -> Self {
        Self {
            document,
            document_revision,
            session_revision,
            locator: node.locator(),
        }
    }

    pub fn for_token(
        document: DocumentId,
        document_revision: DocumentRevision,
        session_revision: u64,
        token: &SyntaxToken<L>,
    ) -> Self {
        Self {
            document,
            document_revision,
            session_revision,
            locator: token.locator(),
        }
    }

    pub fn document(&self) -> DocumentId {
        self.document
    }

    pub fn document_revision(&self) -> DocumentRevision {
        self.document_revision
    }

    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }

    pub fn locator(&self) -> &SyntaxLocator<L> {
        &self.locator
    }

    pub fn resolve_in(
        &self,
        tree: &SyntaxTree<L>,
    ) -> Result<SyntaxElement<L>, RewriteError<L::Kind>> {
        tree.resolve(&self.locator)
    }
}

#[derive(Clone, Debug)]
pub struct SyntaxChangeBatch {
    action: CodeAction,
}

impl SyntaxChangeBatch {
    pub fn new(mut action: CodeAction) -> Result<Self, ActionError> {
        let mut documents = BTreeSet::new();
        let mut changes = action.changes.to_vec();
        for change in &mut changes {
            if !documents.insert(change.document) {
                return Err(ActionError::StructuralInvariant {
                    reason: format!("duplicate document change for {:?}", change.document).into(),
                });
            }
            if let ActionBasis::Document { document, .. } = action.basis {
                if document != change.document {
                    return Err(ActionError::BasisScope {
                        document: change.document,
                    });
                }
            }

            let mut edits = change.edits.to_vec();
            edits.sort_by_key(|edit| (edit.range.start(), edit.range.end()));
            for pair in edits.windows(2) {
                let first = pair[0].range;
                let second = pair[1].range;
                if first.end() > second.start() || first.start() == second.start() {
                    return Err(ActionError::Overlap {
                        document: change.document,
                        first,
                        second,
                    });
                }
            }
            change.edits = edits.into();
        }
        changes.sort_by_key(|change| change.document);
        action.changes = changes.into();
        Ok(Self { action })
    }

    pub fn action(&self) -> &CodeAction {
        &self.action
    }
}

impl crate::edit::sealed::Sealed for SyntaxChangeBatch {}

impl EditBatch for SyntaxChangeBatch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        self.lower_action(context).map_err(Into::into)
    }
}

impl SyntaxChangeBatch {
    fn lower_action(&self, context: EditContext<'_>) -> Result<SourceBundle, ActionError> {
        let context = ActionContext::validate(context)?;
        context.validate_basis(&self.action.basis)?;

        for change in self.action.changes.iter() {
            context.validate_change(change)?;
        }

        let mut candidate = context.source.clone();
        for change in self.action.changes.iter() {
            if change.edits.is_empty() {
                continue;
            }
            let path = context
                .catalog
                .path_for_id(change.document)
                .expect("validated document has a catalog path");
            let text = candidate
                .document_mut(path)
                .expect("validated document has candidate source")
                .text_mut();
            for edit in change.edits.iter().rev() {
                text.replace_range(
                    edit.range.start().to_usize()..edit.range.end().to_usize(),
                    &edit.replacement,
                );
            }
        }
        Ok(candidate)
    }
}

struct ActionContext<'a> {
    source: &'a SourceBundle,
    catalog: &'a Arc<DocumentCatalog>,
    session_revision: u64,
}

impl<'a> ActionContext<'a> {
    fn validate(context: EditContext<'a>) -> Result<Self, ActionError> {
        let catalog = &context.okf_analysis.catalog;
        if catalog.session_revision() != context.session_revision {
            return Err(ActionError::MismatchedAnalysisRevision {
                catalog: catalog.session_revision(),
                requested: context.session_revision,
            });
        }
        if !Arc::ptr_eq(catalog, context.okf_analysis.markdown.catalog())
            || !Arc::ptr_eq(catalog, context.uml.syntax.catalog())
        {
            return Err(ActionError::MismatchedCatalog);
        }
        if context.uml.session_revision() != context.session_revision {
            return Err(ActionError::MismatchedAnalysisRevision {
                catalog: context.uml.session_revision(),
                requested: context.session_revision,
            });
        }
        if catalog.documents().len() != context.source.len() {
            return Err(ActionError::MismatchedCatalog);
        }
        for document in catalog.documents().values() {
            let source = context
                .source
                .document(document.path())
                .ok_or(ActionError::MismatchedCatalog)?;
            if !Arc::ptr_eq(document.text().shared(), source.text_arc()) {
                return Err(ActionError::MismatchedCatalog);
            }
        }
        Ok(Self {
            source: context.source,
            catalog,
            session_revision: context.session_revision,
        })
    }

    fn validate_basis(&self, basis: &ActionBasis) -> Result<(), ActionError> {
        let expected = basis.session_revision();
        if expected != self.session_revision {
            return Err(ActionError::StaleSession {
                expected,
                actual: self.session_revision,
            });
        }
        if let ActionBasis::Document {
            document,
            document_revision,
            ..
        } = basis
        {
            let current = self
                .catalog
                .document(*document)
                .ok_or(ActionError::UnknownDocument {
                    document: *document,
                })?;
            if current.revision() != *document_revision {
                return Err(ActionError::StaleDocument {
                    document: *document,
                    expected: *document_revision,
                    actual: current.revision(),
                });
            }
        }
        Ok(())
    }

    fn validate_change(&self, change: &VersionedDocumentChange) -> Result<(), ActionError> {
        let document =
            self.catalog
                .document(change.document)
                .ok_or(ActionError::UnknownDocument {
                    document: change.document,
                })?;
        if document.revision() != change.base_document_revision {
            return Err(ActionError::StaleDocument {
                document: change.document,
                expected: change.base_document_revision,
                actual: document.revision(),
            });
        }
        let source = self
            .source
            .document(document.path())
            .ok_or(ActionError::MismatchedCatalog)?;
        let len = source.text().len();
        for edit in change.edits.iter() {
            if edit.range.end().to_usize() > len {
                return Err(ActionError::InvalidRange {
                    document: change.document,
                    range: edit.range,
                });
            }
            for offset in [edit.range.start(), edit.range.end()] {
                if !source.text().is_char_boundary(offset.to_usize()) {
                    return Err(ActionError::NonUtf8Boundary {
                        document: change.document,
                        offset,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionError {
    UnknownDocument {
        document: DocumentId,
    },
    StaleSession {
        expected: u64,
        actual: u64,
    },
    StaleDocument {
        document: DocumentId,
        expected: DocumentRevision,
        actual: DocumentRevision,
    },
    DifferentTree {
        document: DocumentId,
    },
    InvalidRange {
        document: DocumentId,
        range: TextRange,
    },
    NonUtf8Boundary {
        document: DocumentId,
        offset: TextSize,
    },
    Overlap {
        document: DocumentId,
        first: TextRange,
        second: TextRange,
    },
    BasisScope {
        document: DocumentId,
    },
    MismatchedCatalog,
    MismatchedAnalysisRevision {
        catalog: u64,
        requested: u64,
    },
    StructuralInvariant {
        reason: Arc<str>,
    },
}

impl fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "syntax action error: {self:?}")
    }
}

impl std::error::Error for ActionError {}

impl crate::edit::EditCoded for ActionError {
    fn edit_code(&self) -> crate::edit::EditCode {
        use crate::edit::EditCode;
        match self {
            ActionError::UnknownDocument { .. } => EditCode::NotFound,
            // Every one of these means "the basis you built this action against
            // is not the basis I am holding" -- re-read and retry, do not
            // reword.
            ActionError::StaleSession { .. }
            | ActionError::StaleDocument { .. }
            | ActionError::DifferentTree { .. }
            | ActionError::MismatchedCatalog
            | ActionError::MismatchedAnalysisRevision { .. } => EditCode::StaleContext,
            ActionError::InvalidRange { .. }
            | ActionError::NonUtf8Boundary { .. }
            | ActionError::Overlap { .. }
            | ActionError::BasisScope { .. } => EditCode::InvalidArgument,
            ActionError::StructuralInvariant { .. } => EditCode::Internal,
        }
    }
}

impl From<ActionError> for EditError {
    fn from(error: ActionError) -> Self {
        EditError::wrap("syntax.action", &error)
    }
}
