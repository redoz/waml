use std::sync::Arc;

use waml_syntax::{AstNode, TextRange, TextSize};

use crate::{
    action::{ActionBasis, ActionError, CodeAction, TextEdit, VersionedDocumentChange},
    source::DocumentId,
    uml::{ActionContext, DeclaredField, ExpectedSyntax},
};
use waml_syntax::DocumentRevision;

pub fn repair_actions(
    context: ActionContext<'_>,
    document: DocumentId,
) -> Result<Vec<CodeAction>, ActionError> {
    let version = context
        .okf()
        .catalog
        .document(document)
        .ok_or(ActionError::UnknownDocument { document })?;
    let Some(snapshot) = context.uml().syntax.document(document) else {
        return Ok(Vec::new());
    };
    if !Arc::ptr_eq(version, snapshot.document()) {
        return Err(ActionError::MismatchedCatalog);
    }
    let concept_id = crate::okf::id_of(version.path().as_str());
    let Some(concept) = context.uml().declared.concept(&concept_id) else {
        return Ok(Vec::new());
    };
    let source = snapshot.syntax().write_to_string();
    let mut actions = Vec::new();
    for attribute in concept.attributes.iter() {
        let colon = attribute.syntax.colon_token();
        if colon.flags().is_missing() {
            let start = colon.range().start();
            let mut end = start.to_usize();
            while source
                .as_bytes()
                .get(end)
                .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
            {
                end += 1;
            }
            actions.push(action(
                "Insert missing `: `",
                &context,
                document,
                version.revision(),
                checked_range(start, size(end)?)?,
                ": ",
            ));
        }
        if matches!(
            attribute.ty,
            DeclaredField::Incomplete {
                expected: ExpectedSyntax::TypeReference,
                ..
            }
        ) && !colon.flags().is_missing()
        {
            let range = attribute.syntax.syntax().range();
            let mut insert = range.end().to_usize();
            while insert > range.start().to_usize()
                && matches!(source.as_bytes()[insert - 1], b'\r' | b'\n' | b' ' | b'\t')
            {
                insert -= 1;
            }
            actions.push(action(
                "Insert missing type `String`",
                &context,
                document,
                version.revision(),
                checked_range(size(insert)?, size(insert)?)?,
                " String",
            ));
        }
        if let DeclaredField::Invalid { syntax, .. } = &attribute.multiplicity {
            let range = syntax.range();
            let raw = &source[range.start().to_usize()..range.end().to_usize()];
            let Some((open, delimiter)) = raw
                .char_indices()
                .find(|(_, delimiter)| matches!(delimiter, '[' | '{'))
            else {
                continue;
            };
            let close_delimiter = if delimiter == '{' { '}' } else { ']' };
            let close = raw[open..]
                .find(close_delimiter)
                .map(|relative| open + relative + 1)
                .unwrap_or(raw.len());
            let number = first_ascii_decimal(&raw[open..close]).unwrap_or("1");
            actions.push(action(
                "Replace invalid multiplicity",
                &context,
                document,
                version.revision(),
                checked_range(
                    size(range.start().to_usize() + open)?,
                    size(range.start().to_usize() + close)?,
                )?,
                &format!("{{{number}}}"),
            ));
        }
    }
    actions.sort_by(|left, right| {
        let left_edit = &left.changes[0].edits[0];
        let right_edit = &right.changes[0].edits[0];
        (
            left_edit.range.start(),
            left_edit.range.end(),
            left.title.as_str(),
        )
            .cmp(&(
                right_edit.range.start(),
                right_edit.range.end(),
                right.title.as_str(),
            ))
    });
    Ok(actions)
}

fn action(
    title: &str,
    context: &ActionContext<'_>,
    document: DocumentId,
    document_revision: DocumentRevision,
    range: TextRange,
    replacement: &str,
) -> CodeAction {
    CodeAction {
        title: title.into(),
        basis: ActionBasis::Document {
            document,
            document_revision,
            session_revision: context.session_revision(),
        },
        changes: Arc::from([VersionedDocumentChange {
            document,
            base_document_revision: document_revision,
            edits: Arc::from([TextEdit {
                range,
                replacement: Arc::from(replacement),
            }]),
        }]),
    }
}
fn size(value: usize) -> Result<TextSize, ActionError> {
    TextSize::try_from_usize(value).map_err(|_| ActionError::StructuralInvariant {
        reason: "repair offset exceeds TextSize".into(),
    })
}
fn checked_range(start: TextSize, end: TextSize) -> Result<TextRange, ActionError> {
    TextRange::new(start, end).map_err(|_| ActionError::StructuralInvariant {
        reason: "repair produced an invalid range".into(),
    })
}
fn first_ascii_decimal(value: &str) -> Option<&str> {
    let start = value.as_bytes().iter().position(u8::is_ascii_digit)?;
    let end = value.as_bytes()[start..]
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map(|relative| start + relative)
        .unwrap_or(value.len());
    Some(&value[start..end])
}
