use std::sync::Arc;

use crate::{
    reparse_okf_markdown_with_structure, DocumentRevision, FullReparseReason,
    MarkdownDialect, MarkdownStructureMap, OkfMarkdownLanguage, OkfSyntaxDiagnosticCode,
    ParseError, ReparseOutcome, SourceText, SyntaxTree, TextChange, TextRange, TreeDiagnostic,
};

#[derive(Default)]
pub struct MarkdownSyntaxQueries;

pub struct MarkdownSyntaxSnapshot {
    revision: DocumentRevision,
    text: SourceText,
    tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    structure: Arc<MarkdownStructureMap>,
    diagnostics: Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
    queries: Arc<MarkdownSyntaxQueries>,
}

impl MarkdownSyntaxSnapshot {
    pub fn revision(&self) -> DocumentRevision { self.revision }
    pub fn text(&self) -> &SourceText { &self.text }
    pub fn tree(&self) -> &Arc<SyntaxTree<OkfMarkdownLanguage>> { &self.tree }
    pub fn structure(&self) -> &Arc<MarkdownStructureMap> { &self.structure }
    pub fn diagnostics(&self) -> &Arc<[TreeDiagnostic<OkfSyntaxDiagnosticCode>]> { &self.diagnostics }
    pub fn queries(&self) -> &Arc<MarkdownSyntaxQueries> { &self.queries }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownReparseOutcome {
    Incremental { shared_source_independent_green: usize, reparsed_range: Option<TextRange> },
    Full { reason: FullReparseReason },
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
    Ok(Arc::new(MarkdownSyntaxSnapshot {
        revision, text, tree: parsed.tree, structure: parsed.structure, diagnostics,
        queries: Arc::new(MarkdownSyntaxQueries),
    }))
}

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError> {
    if revision <= previous.revision {
        return Err(ParseError::NonMonotonicRevision { previous: previous.revision, requested: revision });
    }
    let (outcome, structure) = reparse_okf_markdown_with_structure(previous.tree.as_ref(), new_text.clone(), changes)?;
    let (tree, outcome, affected_ranges): (_, _, Arc<[TextRange]>) = match outcome {
        ReparseOutcome::Incremental { tree, shared_source_independent_green, reparsed_range } => (
            tree,
            MarkdownReparseOutcome::Incremental { shared_source_independent_green, reparsed_range: Some(reparsed_range) },
            Arc::from([reparsed_range]),
        ),
        ReparseOutcome::Full { tree, reason } => (
            tree,
            MarkdownReparseOutcome::Full { reason },
            Arc::from([]),
        ),
    };
    let diagnostics = Arc::from(tree.diagnostics());
    Ok(MarkdownSyntaxUpdate {
        snapshot: Arc::new(MarkdownSyntaxSnapshot {
            revision, text: new_text, tree, structure, diagnostics,
            queries: Arc::new(MarkdownSyntaxQueries),
        }),
        affected_ranges,
        outcome,
    })
}
