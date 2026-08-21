//! Turning a syntax node or declared field into a `Diagnostic`, plus the
//! translation of parser and layout-shape errors into user diagnostics.

use super::syntax_util::items;
use crate::uml::syntax::{self, UmlLanguage};
use crate::{
    analysis::{AnalysisError, DocumentId, DomainAnalysisContext},
    diagnostic::Diagnostic,
};
use waml_syntax::{SyntaxElement, SyntaxNode, SyntaxTree, TextRange};

pub(crate) fn declared_diagnostic(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    syntax: &SyntaxNode<UmlLanguage>,
    code: crate::diagnostic::DiagCode,
    message: String,
    warning: bool,
) -> Result<Diagnostic, AnalysisError> {
    declared_diagnostic_range(context, path, syntax.range(), code, message, warning)
}

pub(crate) fn declared_diagnostic_range(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    range: TextRange,
    code: crate::diagnostic::DiagCode,
    message: String,
    warning: bool,
) -> Result<Diagnostic, AnalysisError> {
    // Seam invariants (declared syntax belongs to a cataloged document): a
    // break becomes an AnalysisError instead of panicking the caller's process.
    let seam = |reason: &str| AnalysisError::CatalogInvariant {
        reason: format!("{reason}: {path}").into(),
    };
    let bundle_path = crate::source::BundlePath::parse(path)
        .map_err(|_| seam("analyzed path is not a valid bundle path"))?;
    let id = context
        .catalog
        .id_for_path(&bundle_path)
        .ok_or_else(|| seam("analyzed path is not cataloged"))?;
    let document = context
        .catalog
        .document(id)
        .ok_or_else(|| seam("cataloged path has no document"))?;
    let line = document
        .line_index()
        .line_col(document.text(), range.start())
        .map_err(|_| seam("declared syntax range is not a document offset"))?;
    let diagnostic = if warning {
        Diagnostic::warn(code, message, path, line.line as usize + 1)
    } else {
        Diagnostic::new(code, message, path, line.line as usize + 1)
    };
    Ok(diagnostic
        .with_span((
            line.byte_column as usize,
            line.byte_column as usize + range.len().to_usize(),
        ))
        .with_provenance(id, document.revision(), range))
}

pub(crate) fn behavior_diagnostic(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    syntax: &SyntaxNode<UmlLanguage>,
    code: crate::diagnostic::DiagCode,
    message: String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Seam invariant (behavior syntax belongs to a cataloged document). This
    // helper fans out through the sequence/flow lowering paths, which have no
    // error channel; on a seam break, drop this one diagnostic instead of
    // panicking the in-process editor / poisoning the wasm instance. Debug
    // builds still assert so tests catch the broken seam.
    let Some(id) = crate::source::BundlePath::parse(path.to_string())
        .ok()
        .and_then(|bundle_path| context.catalog.id_for_path(&bundle_path))
    else {
        debug_assert!(false, "behavior diagnostic path is not cataloged: {path}");
        return;
    };
    let Some(document) = context.catalog.document(id) else {
        debug_assert!(false, "cataloged path has no document: {path}");
        return;
    };
    let range = items(syntax.clone(), syntax::UmlSyntaxKind::Link)
        .into_iter()
        .find_map(|link| {
            link.children()
                .find(|element| element.kind() == syntax::UmlSyntaxKind::LinkTargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .or_else(|| {
            syntax
                .children()
                .find(|element| element.kind() == syntax::UmlSyntaxKind::TargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .unwrap_or_else(|| syntax.range());
    let (Some(start), Some(end)) = (
        document
            .line_index()
            .line_col(document.text(), range.start())
            .ok(),
        document
            .line_index()
            .line_col(document.text(), range.end())
            .ok(),
    ) else {
        debug_assert!(false, "behavior range is not a document offset: {path}");
        return;
    };
    diagnostics.push(
        Diagnostic::new(code, message, path, start.line as usize + 1)
            .with_span((
                start.byte_column as usize,
                if start.line == end.line {
                    end.byte_column as usize
                } else {
                    start.byte_column as usize
                },
            ))
            .with_provenance(id, document.revision(), range),
    );
}

/// Emits a `MalformedLayout` diagnostic for every `layout_fields` entry that
/// failed to resolve cleanly (`Incomplete` or `Invalid`); `Valid` and
/// `Absent` fields need no diagnostic.
///
/// A field the shape parser already rejected carries its own diagnostic, which
/// names the word the grammar wanted and underlines only the malformed run;
/// `translate_parser_diagnostics` forwards those, so this pass skips the field
/// rather than reporting the same line twice.  What is left are fields that
/// went `Invalid` without a shape error -- an empty bullet, an unlexable atom --
/// and those still get the generic message.
pub(crate) fn translate_layout_diagnostics(
    document: &crate::analysis::DocumentVersion,
    id: DocumentId,
    tree: &SyntaxTree<UmlLanguage>,
    layout_fields: &[crate::uml::DeclaredField<UmlLanguage, crate::uml::DeclaredLayoutStatement>],
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), AnalysisError> {
    for field in layout_fields {
        let syntax = match field {
            crate::uml::DeclaredField::Valid { .. } | crate::uml::DeclaredField::Absent => continue,
            crate::uml::DeclaredField::Incomplete { syntax, .. }
            | crate::uml::DeclaredField::Invalid { syntax, .. } => syntax,
        };
        // The statement node, not the recovery node, is what a parser
        // diagnostic sits inside; walk up so containment is decidable.
        let statement = ancestor_or_self(syntax, syntax::UmlSyntaxKind::LayoutStatement);
        let covered = tree.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == syntax::UmlSyntaxDiagnosticCode::MalformedLayout
                && diagnostic.range.start() >= statement.range().start()
                && diagnostic.range.end() <= statement.range().end()
        });
        if covered {
            continue;
        }
        let range = syntax.trimmed_range();
        let start = document
            .line_index()
            .line_col(document.text(), range.start())
            .map_err(|_| AnalysisError::CatalogInvariant {
                reason: "layout diagnostic start is not a document offset".into(),
            })?;
        let end = document
            .line_index()
            .line_col(document.text(), range.end())
            .map_err(|_| AnalysisError::CatalogInvariant {
                reason: "layout diagnostic end is not a document offset".into(),
            })?;
        diagnostics.push(
            Diagnostic::new(
                crate::diagnostic::DiagCode::MalformedLayout,
                "malformed layout statement",
                document.path().as_str(),
                start.line as usize + 1,
            )
            .with_span((
                start.byte_column as usize,
                (if start.line == end.line {
                    end.byte_column
                } else {
                    start.byte_column
                }) as usize,
            ))
            .with_provenance(id, document.revision(), range),
        );
    }
    Ok(())
}

/// The nearest enclosing node of `kind`, or the node itself when it is already
/// that kind or has no such ancestor.
fn ancestor_or_self(
    node: &SyntaxNode<UmlLanguage>,
    kind: syntax::UmlSyntaxKind,
) -> SyntaxNode<UmlLanguage> {
    let mut current = node.clone();
    loop {
        if current.kind() == kind {
            return current;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return node.clone(),
        }
    }
}

pub(crate) fn translate_parser_diagnostics(
    document: &crate::analysis::DocumentVersion,
    id: DocumentId,
    tree: &SyntaxTree<UmlLanguage>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), AnalysisError> {
    for diagnostic in tree.diagnostics() {
        let start = diagnostic.range.start();
        let end = diagnostic.range.end();
        let start_line = document
            .line_index()
            .line_col(document.text(), start)
            .map_err(|_| AnalysisError::CatalogInvariant {
                reason: "parser diagnostic start is not a document offset".into(),
            })?;
        let end_line = document
            .line_index()
            .line_col(document.text(), end)
            .map_err(|_| AnalysisError::CatalogInvariant {
                reason: "parser diagnostic end is not a document offset".into(),
            })?;
        diagnostics.push(
            Diagnostic::new(
                match diagnostic.code {
                    syntax::UmlSyntaxDiagnosticCode::MalformedFlow
                    | syntax::UmlSyntaxDiagnosticCode::MalformedIndentation => {
                        crate::diagnostic::DiagCode::MalformedFlowBullet
                    }
                    syntax::UmlSyntaxDiagnosticCode::MalformedLifeline => {
                        crate::diagnostic::DiagCode::MalformedLifeline
                    }
                    syntax::UmlSyntaxDiagnosticCode::MalformedMessage
                    | syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm => {
                        crate::diagnostic::DiagCode::MalformedMessage
                    }
                    syntax::UmlSyntaxDiagnosticCode::UnresolvedTarget => {
                        crate::diagnostic::DiagCode::UnresolvedTarget
                    }
                    syntax::UmlSyntaxDiagnosticCode::MalformedLayout => {
                        crate::diagnostic::DiagCode::MalformedLayout
                    }
                    // Attribute-line parse errors (missing ':', missing type, an
                    // unparsable multiplicity) and generic parser recovery all
                    // surface on an attribute/member line, so they share
                    // MalformedAttribute.
                    syntax::UmlSyntaxDiagnosticCode::MissingColon
                    | syntax::UmlSyntaxDiagnosticCode::MissingType
                    | syntax::UmlSyntaxDiagnosticCode::InvalidMultiplicity
                    | syntax::UmlSyntaxDiagnosticCode::UnexpectedToken => {
                        crate::diagnostic::DiagCode::MalformedAttribute
                    }
                },
                diagnostic.message.to_string(),
                document.path().as_str(),
                start_line.line as usize + 1,
            )
            .with_span((
                start_line.byte_column as usize,
                (if start_line.line == end_line.line {
                    end_line.byte_column
                } else {
                    start_line.byte_column
                }) as usize,
            ))
            .with_provenance(id, document.revision(), diagnostic.range),
        );
    }
    Ok(())
}
