use std::{collections::HashMap, sync::Arc};

use crate::{SyntaxIdentity, TextRange};

#[derive(Clone, Debug)]
pub struct MarkdownReferenceDefinition {
    pub label: Arc<str>,
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub destination: Arc<str>,
    pub destination_range: TextRange,
    pub title: Option<Arc<str>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MarkdownReferenceMap {
    pub(crate) definitions: Arc<HashMap<Arc<str>, MarkdownReferenceDefinition>>,
    pub(crate) backlinks: Arc<HashMap<Arc<str>, Arc<[SyntaxIdentity]>>>,
}

impl MarkdownReferenceMap {
    pub(crate) fn from_source(source: &str) -> Result<Self, crate::ParseError> {
        let mut definitions = HashMap::new();
        let mut at = 0;
        for line in source.split_inclusive('\n') {
            let end = at + line.len();
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if let Some(close) = trimmed.find("]:") {
                if trimmed.starts_with('[') {
                    let label = &trimmed[1..close];
                    if let Some(normalized) = normalize_label(label) {
                        let value = trimmed[close + 2..].trim_start();
                        let value_offset = trimmed.len() - value.len();
                        let destination = value
                            .strip_prefix('<')
                            .and_then(|value| {
                                value.split_once('>').map(|(destination, _)| destination)
                            })
                            .unwrap_or_else(|| value.split_whitespace().next().unwrap_or_default());
                        let destination_start =
                            at + value_offset + usize::from(value.starts_with('<'));
                        let definition = MarkdownReferenceDefinition {
                            label: normalized.clone(),
                            owner: SyntaxIdentity::fresh()?,
                            source_range: range(at, end)?,
                            destination: destination.into(),
                            destination_range: range(
                                destination_start,
                                destination_start + destination.len(),
                            )?,
                            title: None,
                        };
                        definitions.entry(normalized).or_insert(definition);
                    }
                }
            }
            at = end;
        }
        Ok(Self {
            definitions: Arc::new(definitions),
            backlinks: Arc::new(HashMap::new()),
        })
    }
}

pub(crate) fn normalize_label(label: &str) -> Option<Arc<str>> {
    let normalized = label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    (!normalized.is_empty() && normalized.chars().count() <= 999).then(|| normalized.into())
}

fn range(start: usize, end: usize) -> Result<TextRange, crate::ParseError> {
    let start = crate::TextSize::try_from_usize(start)
        .map_err(|_| crate::ParseError::SourceTooLarge { bytes: start })?;
    let end = crate::TextSize::try_from_usize(end)
        .map_err(|_| crate::ParseError::SourceTooLarge { bytes: end })?;
    TextRange::new(start, end).map_err(|_| crate::ParseError::StructuralInvariant {
        reason: "reversed reference range".into(),
    })
}
