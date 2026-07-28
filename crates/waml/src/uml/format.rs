use std::{fmt, sync::Arc};

use waml_syntax::{TextRange, TextSize};

use crate::{
    action::{ActionBasis, ActionError, CodeAction, TextEdit, VersionedDocumentChange},
    analysis::{DocumentId, OkfAnalysis, PreparedCandidate},
    edit::EditError,
    uml,
};

pub struct ActionContext<'a> {
    okf: &'a OkfAnalysis,
    uml: &'a uml::Analysis,
    session_revision: u64,
}

impl<'a> ActionContext<'a> {
    pub fn new(
        okf: &'a OkfAnalysis,
        uml: &'a uml::Analysis,
        session_revision: u64,
    ) -> Result<Self, ActionError> {
        if !Arc::ptr_eq(&okf.catalog, uml.syntax.catalog()) {
            return Err(ActionError::MismatchedCatalog);
        }
        for revision in [
            okf.catalog.session_revision(),
            uml.syntax.catalog().session_revision(),
            uml.session_revision(),
        ] {
            if revision != session_revision {
                return Err(ActionError::MismatchedAnalysisRevision {
                    catalog: revision,
                    requested: session_revision,
                });
            }
        }
        Ok(Self {
            okf,
            uml,
            session_revision,
        })
    }

    pub fn from_prepared(candidate: &'a PreparedCandidate) -> Result<Self, ActionError> {
        Self::new(candidate.okf(), candidate.uml(), candidate.revision())
    }
    pub fn okf(&self) -> &'a OkfAnalysis {
        self.okf
    }
    pub fn uml(&self) -> &'a uml::Analysis {
        self.uml
    }
    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }
}

pub struct Formatter;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormatError {
    Action(ActionError),
    UnknownDocument { document: DocumentId },
    NotClaimed { document: DocumentId },
    StructuralInvariant { reason: Arc<str> },
}
impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "UML format error: {self:?}")
    }
}
impl std::error::Error for FormatError {}
impl From<ActionError> for FormatError {
    fn from(error: ActionError) -> Self {
        Self::Action(error)
    }
}
impl From<FormatError> for EditError {
    fn from(error: FormatError) -> Self {
        EditError {
            index: 0,
            op: "uml.format".into(),
            selector: None,
            reason: error.to_string(),
        }
    }
}

impl Formatter {
    pub fn format(
        &self,
        context: ActionContext<'_>,
        document: DocumentId,
    ) -> Result<CodeAction, FormatError> {
        let version = context
            .okf
            .catalog
            .document(document)
            .ok_or(FormatError::UnknownDocument { document })?;
        let snapshot = context
            .uml
            .syntax
            .document(document)
            .ok_or(FormatError::NotClaimed { document })?;
        if !Arc::ptr_eq(version, snapshot.document()) {
            return Err(FormatError::StructuralInvariant {
                reason: "UML syntax snapshot does not share the catalog document".into(),
            });
        }
        let exact = snapshot.syntax().write_to_string();
        let has_recovery = !snapshot.syntax().diagnostics().is_empty();
        let edits = if has_recovery {
            Vec::new()
        } else {
            canonical_whitespace_edits(&exact)?
        };
        Ok(CodeAction {
            title: "Format UML document".into(),
            basis: ActionBasis::Document {
                document,
                document_revision: version.revision(),
                session_revision: context.session_revision,
            },
            changes: Arc::from([VersionedDocumentChange {
                document,
                base_document_revision: version.revision(),
                edits: edits.into(),
            }]),
        })
    }
}

fn canonical_whitespace_edits(source: &str) -> Result<Vec<TextEdit>, FormatError> {
    const OWNED_HEADINGS: &[&str] = &[
        "## Attributes",
        "## Slots",
        "## Values",
        "## Relationships",
        "## Notes",
        "## Lifelines",
        "## Messages",
        "## Members",
        "## Layout",
        "## Body",
    ];
    let mut edits = Vec::new();
    if source.starts_with("---") {
        let mut delimiters = source.match_indices("---");
        let _opening = delimiters.next();
        if let Some((closing, _)) = delimiters.next() {
            let after = closing + 3;
            let newline_len = if source[after..].starts_with("\r\n") {
                2
            } else if source[after..].starts_with('\n') {
                1
            } else {
                0
            };
            let insertion = after + newline_len;
            if newline_len != 0
                && !source[insertion..].starts_with('\n')
                && !source[insertion..].starts_with("\r\n")
            {
                edits.push(TextEdit {
                    range: TextRange::new(size(insertion)?, size(insertion)?)
                        .map_err(|_| invariant("invalid range"))?,
                    replacement: Arc::from(if newline_len == 2 { "\r\n" } else { "\n" }),
                });
            }
        }
    }
    let mut offset = 0usize;
    let mut owned = false;
    let mut previous_was_heading = false;
    for line in source.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content.starts_with("## ") {
            owned = OWNED_HEADINGS.contains(&content);
            previous_was_heading = owned;
        } else if content.starts_with("# ") {
            owned = false;
            previous_was_heading = false;
        } else if content.is_empty() && previous_was_heading && owned {
            let start = size(offset)?;
            let end = size(offset + line.len())?;
            edits.push(TextEdit {
                range: TextRange::new(start, end).map_err(|_| invariant("invalid range"))?,
                replacement: Arc::from(""),
            });
        } else if !content.is_empty() {
            previous_was_heading = content.starts_with("### ") && owned;
        }
        offset += line.len();
    }
    Ok(edits)
}

fn size(value: usize) -> Result<TextSize, FormatError> {
    TextSize::try_from_usize(value).map_err(|_| invariant("offset exceeds TextSize"))
}
fn invariant(reason: &str) -> FormatError {
    FormatError::StructuralInvariant {
        reason: Arc::from(reason),
    }
}
