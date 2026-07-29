use crate::edit::EditError;
use crate::source::{BundlePath, SourceBundle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TextSplice {
    path: BundlePath,
    start: usize,
    removed: String,
    inserted: String,
}

pub(crate) struct DeltaBatch(pub(crate) SourceDelta);

impl crate::edit::sealed::Sealed for DeltaBatch {}

impl crate::edit::EditBatch for DeltaBatch {
    fn lower(&self, context: crate::edit::EditContext<'_>) -> Result<SourceBundle, EditError> {
        self.0.apply(context.source).map(|(source, _)| source)
    }

    fn apply_reversible(
        &self,
        context: crate::edit::EditContext<'_>,
    ) -> Result<crate::edit::AppliedEdit, EditError> {
        let (source, inverse) = self.0.apply(context.source)?;
        Ok(crate::edit::AppliedEdit {
            source,
            inverse: crate::edit::PendingEdit::from_delta(inverse),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceDelta {
    Text(Vec<TextSplice>),
    Structural {
        before: SourceBundle,
        after: SourceBundle,
    },
}

impl SourceDelta {
    pub(crate) fn between(before: &SourceBundle, after: &SourceBundle) -> Self {
        let same_document_layout = before.len() == after.len()
            && before
                .documents()
                .iter()
                .zip(after.documents())
                .all(|(before, after)| before.path() == after.path());
        if !same_document_layout {
            return Self::Structural {
                before: before.clone(),
                after: after.clone(),
            };
        }

        let changes = before
            .documents()
            .iter()
            .zip(after.documents())
            .filter_map(|(before, after)| {
                text_splice(before.path().clone(), before.text(), after.text())
            })
            .collect();

        Self::Text(changes)
    }

    pub(crate) fn apply(
        &self,
        source: &SourceBundle,
    ) -> Result<(SourceBundle, SourceDelta), EditError> {
        match self {
            SourceDelta::Text(changes) => apply_text_changes(changes, source),
            SourceDelta::Structural { before, after } => {
                if source != before {
                    return Err(EditError::at(
                        "history.restore",
                        "source bundle no longer matches structural history",
                    ));
                }
                Ok((
                    after.clone(),
                    SourceDelta::Structural {
                        before: after.clone(),
                        after: before.clone(),
                    },
                ))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn stored_text_bytes(&self) -> usize {
        match self {
            SourceDelta::Text(changes) => changes
                .iter()
                .map(|change| change.removed.len() + change.inserted.len())
                .sum(),
            SourceDelta::Structural { .. } => 0,
        }
    }
}

fn apply_text_changes(
    changes: &[TextSplice],
    source: &SourceBundle,
) -> Result<(SourceBundle, SourceDelta), EditError> {
    let mut candidate = source.clone();
    let mut reciprocal = Vec::with_capacity(changes.len());

    for change in changes {
        let document = candidate.document_mut(&change.path).ok_or_else(|| {
            EditError::at(
                "history.restore",
                format!("missing document '{}'", change.path),
            )
        })?;
        let end = change.start + change.removed.len();
        let matches_preimage = document
            .text()
            .get(change.start..end)
            .is_some_and(|text| text == change.removed);
        if !matches_preimage {
            return Err(EditError::at(
                "history.restore",
                format!("document '{}' no longer matches history", change.path),
            ));
        }

        document
            .text_mut()
            .replace_range(change.start..end, &change.inserted);
        reciprocal.push(TextSplice {
            path: change.path.clone(),
            start: change.start,
            removed: change.inserted.clone(),
            inserted: change.removed.clone(),
        });
    }

    Ok((candidate, SourceDelta::Text(reciprocal)))
}

fn text_splice(path: BundlePath, before: &str, after: &str) -> Option<TextSplice> {
    if before == after {
        return None;
    }

    let prefix = common_prefix_bytes(before, after);
    let suffix = before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
        .take_while(|(before, after)| before == after)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();

    Some(TextSplice {
        path,
        start: prefix,
        removed: before[prefix..before.len() - suffix].to_string(),
        inserted: after[prefix..after.len() - suffix].to_string(),
    })
}

fn common_prefix_bytes(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(character, _)| character.len_utf8())
        .sum()
}
