use std::sync::Arc;

use crate::{
    reparse_okf_markdown_with_structure, DocumentRevision, FullReparseReason, MarkdownDialect,
    MarkdownStructureMap, OkfMarkdownLanguage, OkfSyntaxDiagnosticCode, ParseError, ReparseOutcome,
    SourceText, SyntaxTree, TextChange, TextRange, TreeDiagnostic,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownLinkKind {
    Inline,
    Reference,
}

#[derive(Clone, Debug)]
pub struct MarkdownLink {
    pub destination: Arc<str>,
    pub destination_range: Option<TextRange>,
    pub kind: MarkdownLinkKind,
}

#[derive(Default)]
pub struct MarkdownSyntaxQueries {
    links: Arc<[MarkdownLink]>,
}
impl MarkdownSyntaxQueries {
    pub fn links(&self) -> impl Iterator<Item = &MarkdownLink> {
        self.links.iter()
    }
}

pub struct MarkdownSyntaxSnapshot {
    revision: DocumentRevision,
    text: SourceText,
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    structure: Arc<MarkdownStructureMap>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    queries: Arc<MarkdownSyntaxQueries>,
}

impl MarkdownSyntaxSnapshot {
    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }
    pub fn text(&self) -> &SourceText {
        &self.text
    }
    pub fn tree(&self) -> &Arc<SyntaxTree<OkfMarkdownLanguage>> {
        &self.tree
    }
    pub fn structure(&self) -> &Arc<MarkdownStructureMap> {
        &self.structure
    }
    pub fn diagnostics(&self) -> &Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]> {
        &self.diagnostics
    }
    pub fn queries(&self) -> &Arc<MarkdownSyntaxQueries> {
        &self.queries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownReparseOutcome {
    Incremental {
        shared_source_independent_green: usize,
        reparsed_range: Option<TextRange>,
    },
    Full {
        reason: FullReparseReason,
    },
}

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}

pub fn parse_markdown(
    revision: DocumentRevision,
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<Arc<MarkdownSyntaxSnapshot>, ParseError> {
    let parsed = crate::parse_okf_markdown(text.clone(), dialect)?;
    let diagnostics = Arc::from(parsed.tree.diagnostics());
    let queries = Arc::new(queries(text.shared())?);
    Ok(Arc::new(MarkdownSyntaxSnapshot {
        revision,
        text,
        tree: parsed.tree,
        structure: parsed.structure,
        diagnostics,
        queries,
    }))
}

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError> {
    if revision <= previous.revision {
        return Err(ParseError::NonMonotonicRevision {
            previous: previous.revision,
            requested: revision,
        });
    }
    let (outcome, structure) =
        reparse_okf_markdown_with_structure(previous.tree.as_ref(), new_text.clone(), changes)?;
    let (tree, outcome, affected_ranges): (_, _, Arc<[TextRange]>) = match outcome {
        ReparseOutcome::Incremental {
            tree,
            shared_source_independent_green,
            reparsed_range,
        } => {
            if reparsed_range.end() > new_text.len() {
                return Err(ParseError::StructuralInvariant {
                    reason: "incremental reparse range exceeds the new Markdown snapshot".into(),
                });
            }
            (
                tree,
                MarkdownReparseOutcome::Incremental {
                    shared_source_independent_green,
                    reparsed_range: Some(reparsed_range),
                },
                Arc::from([reparsed_range]),
            )
        }
        ReparseOutcome::Full { tree, reason } => {
            (tree, MarkdownReparseOutcome::Full { reason }, Arc::from([]))
        }
    };
    let diagnostics = Arc::from(tree.diagnostics());
    let queries = Arc::new(queries(new_text.shared())?);
    Ok(MarkdownSyntaxUpdate {
        snapshot: Arc::new(MarkdownSyntaxSnapshot {
            revision,
            text: new_text,
            tree,
            structure,
            diagnostics,
            queries,
        }),
        affected_ranges,
        outcome,
    })
}

fn queries(source: &str) -> Result<MarkdownSyntaxQueries, ParseError> {
    let references = super::reference::MarkdownReferenceMap::from_source(source)?;
    let mut links = Vec::new();
    let mut at = 0;
    while let Some(relative) = source[at..].find('[') {
        let open = at + relative;
        let Some(label_end_relative) = source[open + 1..].find(']') else {
            break;
        };
        let label_end = open + 1 + label_end_relative;
        let label = &source[open + 1..label_end];
        let after = label_end + 1;
        if source[after..].starts_with(':') {
            at = after;
            continue;
        }
        if source[after..].starts_with('(') {
            if let Some(close_relative) = source[after + 1..].find(')') {
                let destination = &source[after + 1..after + 1 + close_relative];
                links.push(MarkdownLink {
                    destination: destination.into(),
                    destination_range: Some(range(after + 1, after + 1 + destination.len())?),
                    kind: MarkdownLinkKind::Inline,
                });
                at = after + close_relative + 2;
                continue;
            }
        }
        let (reference_label, next) = if source[after..].starts_with('[') {
            let Some(close_relative) = source[after + 1..].find(']') else {
                at = after;
                continue;
            };
            let end = after + close_relative + 2;
            let value = &source[after + 1..end - 1];
            (if value.is_empty() { label } else { value }, end)
        } else {
            (label, after)
        };
        if let Some(normalized) = super::reference::normalize_label(reference_label) {
            if let Some(definition) = references.definitions.get(&normalized) {
                links.push(MarkdownLink {
                    destination: definition.destination.clone(),
                    destination_range: Some(definition.destination_range),
                    kind: MarkdownLinkKind::Reference,
                });
            }
        }
        at = next;
    }
    Ok(MarkdownSyntaxQueries {
        links: links.into(),
    })
}

fn range(start: usize, end: usize) -> Result<TextRange, ParseError> {
    let start = crate::TextSize::try_from_usize(start)
        .map_err(|_| ParseError::SourceTooLarge { bytes: start })?;
    let end = crate::TextSize::try_from_usize(end)
        .map_err(|_| ParseError::SourceTooLarge { bytes: end })?;
    TextRange::new(start, end).map_err(|_| ParseError::StructuralInvariant {
        reason: "reversed link range".into(),
    })
}
